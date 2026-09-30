// SplatCrab desktop.
//
// Five regions over the evaluation protocol (docs/modules/U0-ui-foundations.md,
// docs/modules/U2-ui-desktop.md and docs/modules/U3-ui-editor.md): in the
// middle the editor above the command window, the file browser on the left,
// and the workspace above the command history on the right, with a splitter
// between each pair.
//
// Every request is one POST /api carrying the session token from this
// page's URL fragment in the X-SplatCrab-Token header, and every request
// goes through one queue, one at a time in the order the page made them, so
// the panes' refreshes never race the command window, the editor or each
// other.
//
// Everything a request returns reaches the page as text (textContent,
// createTextNode and the editor's value), never as markup: variable names,
// previews, file names, file text, paths, messages, frame names and history
// entries are the user's own. The page's Content-Security-Policy forbids
// inline script and style, which is why this file is separate and why pane
// sizes and the gutter's mark are custom properties set through the CSS
// object model rather than style attributes. The editor asks its questions
// in the page itself, never in a dialog of the browser's.

'use strict';

(function () {
  // A fragment that is not valid percent-encoding is taken as it stands,
  // rather than stopping the page before it can say anything.
  const token = (function (raw) {
    try {
      return decodeURIComponent(raw);
    } catch (e) {
      return raw;
    }
  })(location.hash.slice(1));

  function byId(id) {
    return document.getElementById(id);
  }

  const desktop = byId('desktop');
  const middle = byId('middle');
  const side = byId('side');
  const transcript = byId('transcript');
  const input = byId('input');
  const filesPath = byId('files-path');
  const filesList = byId('files-list');
  const varsBody = byId('vars-body');
  const historyList = byId('history-list');
  const historyBody = byId('history-body');
  const splitLeft = byId('split-left');
  const splitRight = byId('split-right');
  const splitSide = byId('split-side');
  const splitMid = byId('split-mid');
  const editor = byId('editor');
  const tabBar = byId('tabs');
  const code = byId('code');
  const gutter = byId('gutter');
  const gutterLines = byId('gutter-lines');
  const gutterMark = byId('gutter-mark');
  const editorBody = byId('editor-body');
  const editorHint = byId('editor-hint');
  const editorNote = byId('editor-note');
  const newButton = byId('new-file');
  const saveButton = byId('save-file');
  const runButton = byId('run-file');
  const runSelectionButton = byId('run-selection');
  const saveAs = byId('save-as');
  const savePath = byId('save-path');
  const saveAsOk = byId('save-as-ok');
  const saveAsCancel = byId('save-as-cancel');
  const closeAsk = byId('close-ask');
  const closeText = byId('close-text');
  const closeSave = byId('close-save');
  const closeDiscard = byId('close-discard');
  const closeCancel = byId('close-cancel');

  // ---- Requests ----------------------------------------------------------

  let nextId = 1;
  let queue = Promise.resolve();

  // One protocol request, sent now. A refusal by the server (403, 413, ...)
  // is thrown with the server's own status text as its message.
  async function send(request) {
    const reply = await fetch('/api', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'X-SplatCrab-Token': token,
      },
      body: JSON.stringify(request),
      cache: 'no-store',
    });
    const text = await reply.text();
    if (!reply.ok) {
      throw new Error(text);
    }
    return JSON.parse(text);
  }

  // The one way a request leaves the page: queued behind every request made
  // before it, whatever became of those.
  function call(request) {
    request.id = nextId++;
    const answer = queue.then(function () {
      return send(request);
    });
    queue = answer.catch(function () {
      return undefined;
    });
    return answer;
  }

  function element(tag, className, text) {
    const el = document.createElement(tag);
    if (className) {
      el.className = className;
    }
    if (text !== undefined) {
      el.textContent = text;
    }
    return el;
  }

  // ---- The command window ------------------------------------------------

  // The shared history, oldest first: what the history pane lists and what
  // Up and Down walk.
  const history = [];
  let place = 0; // index into history; history.length is the draft
  let draft = '';
  let busy = false;

  function block(className, text) {
    return element('pre', className, text);
  }

  function scrollToEnd() {
    transcript.scrollTop = transcript.scrollHeight;
  }

  function note(text) {
    const entry = element('div', 'entry');
    entry.appendChild(block('err', text));
    transcript.appendChild(entry);
    scrollToEnd();
  }

  function resize() {
    input.rows = Math.min(12, input.value.split('\n').length);
  }

  function setInput(text) {
    input.value = text;
    input.selectionStart = input.selectionEnd = text.length;
    resize();
  }

  function insertNewline() {
    input.setRangeText('\n', input.selectionStart, input.selectionEnd, 'end');
    resize();
  }

  // An error set apart: its message, and under it each frame of its stack,
  // innermost first, as the terminal's trace writes them. A frame whose
  // file is under the file root is a link that opens the file at its line.
  function errorBlock(error) {
    const pre = block('err', 'Error: ' + error.message);
    (error.stack || []).forEach(function (frame) {
      const text = 'in ' + frame.name + (frame.line === null ? '' : ' (line ' + frame.line + ')');
      pre.appendChild(document.createTextNode('\n  '));
      if (frame.file === null) {
        pre.appendChild(document.createTextNode(text));
        return;
      }
      const link = element('button', 'link', text);
      link.type = 'button';
      link.title = frame.file;
      link.addEventListener('click', function () {
        openFile(frame.file, frame.line);
      });
      pre.appendChild(link);
    });
    return pre;
  }

  // Adds an entry to the shared history, as it is run.
  function remember(entry) {
    call({ op: 'history_add', entry: entry })
      .then(function (answer) {
        if (answer.ok && answer.added) {
          addHistory(entry);
        } else if (!answer.ok) {
          historyNote(answer.error.message);
        }
      })
      .catch(function () {});
  }

  // Runs one entry: `shown` in the transcript, then its output exactly as
  // the interpreter wrote it, then its error, if any, set apart, the
  // request being `request`. The entry goes to the shared history as shown
  // as it is run, and the workspace and the file browser are refreshed
  // afterwards, whatever the evaluation answered. Resolves to the answer,
  // or null when the server refused the request.
  async function runEntry(shown, request) {
    const entry = element('div', 'entry');
    entry.appendChild(block('in', shown));
    transcript.appendChild(entry);
    scrollToEnd();
    remember(shown);
    let answer = null;
    try {
      answer = await call(request);
      if (answer.out) {
        entry.appendChild(block('out', answer.out));
      }
      if (!answer.ok) {
        entry.appendChild(errorBlock(answer.error));
      }
    } catch (e) {
      entry.appendChild(block('err', e.message));
    }
    scrollToEnd();
    refreshWorkspace();
    refreshFiles();
    return answer;
  }

  // An entry typed in the command window, its errors with their frames.
  function run(entry) {
    return runEntry(entry, { op: 'eval', code: entry, stack: true });
  }

  async function enter() {
    const entry = input.value;
    if (busy || entry.trim() === '') {
      return;
    }
    busy = true;
    input.readOnly = true;
    try {
      const answer = await call({ op: 'complete', code: entry });
      if (!answer.ok) {
        note('Error: ' + answer.error.message);
        return;
      }
      if (!answer.complete) {
        input.readOnly = false;
        insertNewline();
        return;
      }
      place = history.length;
      draft = '';
      setInput('');
      await run(entry);
    } catch (e) {
      note(e.message);
    } finally {
      busy = false;
      input.readOnly = false;
      input.focus();
    }
  }

  function walk(step) {
    if (place === history.length) {
      draft = input.value;
    }
    place = Math.max(0, Math.min(history.length, place + step));
    setInput(place === history.length ? draft : history[place]);
  }

  function onFirstLine() {
    return input.value.lastIndexOf('\n', input.selectionStart - 1) === -1;
  }

  function onLastLine() {
    return input.value.indexOf('\n', input.selectionEnd) === -1;
  }

  input.addEventListener('keydown', function (ev) {
    if (ev.key === 'Enter' && !ev.shiftKey && !ev.isComposing) {
      ev.preventDefault();
      enter();
    } else if (ev.key === 'ArrowUp' && !busy && onFirstLine()) {
      ev.preventDefault();
      walk(-1);
    } else if (ev.key === 'ArrowDown' && !busy && onLastLine()) {
      ev.preventDefault();
      walk(1);
    }
  });

  input.addEventListener('input', resize);

  // A click in the transcript returns focus to the input, unless it was
  // made to select text there.
  transcript.addEventListener('mouseup', function () {
    if (window.getSelection().isCollapsed) {
      input.focus();
    }
  });

  // ---- The workspace -----------------------------------------------------

  function cell(text, className) {
    const td = element('td', className, text);
    td.title = text;
    return td;
  }

  function showVars(vars) {
    varsBody.replaceChildren();
    if (vars.length === 0) {
      const tr = element('tr');
      const td = element('td', 'note', 'No variables.');
      td.colSpan = 4;
      tr.appendChild(td);
      varsBody.appendChild(tr);
      return;
    }
    vars.forEach(function (v) {
      const tr = element('tr');
      tr.appendChild(cell(v.name, 'name'));
      tr.appendChild(cell(v.value, 'value'));
      tr.appendChild(cell(v.size[0] + '×' + v.size[1], 'size'));
      tr.appendChild(cell(v['class'], 'class'));
      varsBody.appendChild(tr);
    });
  }

  function refreshWorkspace() {
    return call({ op: 'workspace', preview: true })
      .then(function (answer) {
        if (answer.ok) {
          showVars(answer.vars);
        }
      })
      .catch(function () {});
  }

  // ---- The file browser --------------------------------------------------

  // The folder listed, relative to the file root and normalised as the
  // server answered it: '' is the root.
  let folder = '';

  function joined(path, name) {
    return path === '' ? name : path + '/' + name;
  }

  function parent(path) {
    const at = path.lastIndexOf('/');
    return at === -1 ? '' : path.slice(0, at);
  }

  function lastName(path) {
    return path.slice(path.lastIndexOf('/') + 1);
  }

  function where(root, path) {
    if (root === '/') {
      return '/' + path;
    }
    return path === '' ? root : root + '/' + path;
  }

  function bytes(n) {
    if (typeof n !== 'number') {
      return '';
    }
    if (n < 1024) {
      return n + ' B';
    }
    const units = ['KB', 'MB', 'GB', 'TB'];
    let v = n / 1024;
    let u = 0;
    while (v >= 1024 && u < units.length - 1) {
      v /= 1024;
      u += 1;
    }
    return v.toFixed(1) + ' ' + units[u];
  }

  function folderItem(label, path, up) {
    const li = element('li');
    const button = element('button', up ? 'name folder up' : 'name folder', label);
    button.type = 'button';
    button.title = label;
    button.addEventListener('click', function () {
      listFolder(path, true);
    });
    li.appendChild(button);
    return li;
  }

  // A file of the listing: a double click, or Enter once it is focused,
  // opens it in the editor.
  function fileItem(path, name, size) {
    const li = element('li');
    const button = element('button', 'name file', name);
    button.type = 'button';
    button.title = name + ': double-click to open';
    button.addEventListener('dblclick', function () {
      openFile(path, null);
    });
    button.addEventListener('keydown', function (ev) {
      if (ev.key === 'Enter') {
        ev.preventDefault();
        openFile(path, null);
      }
    });
    li.appendChild(button);
    li.appendChild(element('span', 'size', bytes(size)));
    return li;
  }

  function showFiles(answer) {
    filesPath.textContent = where(answer.root, answer.path);
    filesList.replaceChildren();
    // The way up is the page's own, and there is none at the root.
    if (answer.path !== '') {
      filesList.appendChild(folderItem('..', parent(answer.path), true));
    }
    answer.entries.forEach(function (e) {
      const path = joined(answer.path, e.name);
      filesList.appendChild(e.dir ? folderItem(e.name, path, false) : fileItem(path, e.name, e.size));
    });
    if (answer.entries.length === 0) {
      filesList.appendChild(element('li', 'note', 'Empty folder.'));
    }
    if (answer.truncated) {
      filesList.appendChild(
        element('li', 'note', 'Only the first ' + answer.entries.length + ' entries are shown.')
      );
    }
  }

  function showFilesError(text) {
    filesList.replaceChildren(element('li', 'note', text));
  }

  // Lists `path`; when it cannot be listed, most often because code
  // deleted it, falls back to the root once.
  function listFolder(path, fallBack) {
    return call({ op: 'files', path: path })
      .then(function (answer) {
        if (answer.ok) {
          folder = answer.path;
          showFiles(answer);
        } else if (fallBack && path !== '') {
          return listFolder('', false);
        } else {
          showFilesError(answer.error.message);
        }
        return undefined;
      })
      .catch(function (e) {
        showFilesError(e.message);
      });
  }

  function refreshFiles() {
    return listFolder(folder, true);
  }

  // ---- The editor --------------------------------------------------------

  // One tab per open file. `path` is the file's path relative to the file
  // root, and '' for a tab never saved; `saved` is its text as last read or
  // written, with the LF line ends the editor holds; `crlf` is whether the
  // file had CRLF line ends, which a save puts back; `value`, the cursor
  // and the scroll are kept while another tab is shown; `mark` is the line
  // an error was raised on, marked in the gutter until the next edit, or 0.
  const tabs = [];
  let active = null;
  // Escape was pressed in the editor: the next Tab moves the focus on.
  let tabMoves = false;
  // How many lines the gutter numbers.
  let gutterCount = 0;
  // A Run or a Run Selection in progress, so a second waits for none.
  let editorBusy = false;
  // The question the save-as field is answering, and the tab a close is
  // asking about.
  let pathAsked = null;
  let closing = null;

  const INDENT = '    ';

  function makeTab(path, name, text, crlf) {
    return {
      path: path,
      name: name,
      value: text,
      saved: text,
      crlf: crlf,
      start: 0,
      end: 0,
      top: 0,
      left: 0,
      mark: 0,
      shownDirty: false,
    };
  }

  function textOf(tab) {
    return tab === active ? code.value : tab.value;
  }

  function isDirty(tab) {
    return textOf(tab) !== tab.saved;
  }

  function say(text) {
    editorNote.textContent = text;
  }

  function lineCount(text) {
    let n = 1;
    let at = text.indexOf('\n');
    while (at !== -1) {
      n += 1;
      at = text.indexOf('\n', at + 1);
    }
    return n;
  }

  function lineStart(text, at) {
    return at === 0 ? 0 : text.lastIndexOf('\n', at - 1) + 1;
  }

  function lineHeight() {
    return parseFloat(window.getComputedStyle(code).lineHeight) || 20;
  }

  // The gutter's numbers, rebuilt only when the number of lines changes,
  // and its scroll kept with the text's.
  function drawGutter(force) {
    const n = lineCount(code.value);
    if (force || n !== gutterCount) {
      gutterCount = n;
      const numbers = new Array(n);
      for (let k = 0; k < n; k += 1) {
        numbers[k] = String(k + 1);
      }
      gutterLines.textContent = numbers.join('\n');
    }
    gutter.scrollTop = code.scrollTop;
  }

  function drawMark() {
    if (active !== null && active.mark > 0) {
      gutter.style.setProperty('--mark-row', String(active.mark - 1));
      gutterMark.hidden = false;
    } else {
      gutterMark.hidden = true;
    }
  }

  function drawTools() {
    const none = active === null;
    saveButton.disabled = none;
    runButton.disabled = none;
    runSelectionButton.disabled = none;
  }

  function drawTabs() {
    tabBar.replaceChildren();
    tabs.forEach(function (tab) {
      tab.shownDirty = isDirty(tab);
      const holder = element('div', tab === active ? 'tab active' : 'tab');
      const name = element('button', 'tab-name', tab.name);
      name.type = 'button';
      name.setAttribute('role', 'tab');
      name.setAttribute('aria-selected', tab === active ? 'true' : 'false');
      name.title = tab.path === '' ? tab.name + ', not saved yet' : tab.path;
      if (tab.shownDirty) {
        name.appendChild(element('span', 'dirty', ' •'));
      }
      name.addEventListener('click', function () {
        activate(tab);
      });
      const close = element('button', 'tab-close', '×');
      close.type = 'button';
      close.title = 'Close ' + tab.name;
      close.setAttribute('aria-label', 'Close ' + tab.name);
      close.addEventListener('click', function () {
        askClose(tab);
      });
      holder.appendChild(name);
      holder.appendChild(close);
      tabBar.appendChild(holder);
    });
    drawTools();
  }

  function showEditor() {
    const open = tabs.length > 0;
    editorBody.hidden = !open;
    editorHint.hidden = open;
  }

  // Keeps what the active tab shows, while another is shown.
  function stash() {
    if (active !== null) {
      active.value = code.value;
      active.start = code.selectionStart;
      active.end = code.selectionEnd;
      active.top = code.scrollTop;
      active.left = code.scrollLeft;
    }
  }

  function activate(tab) {
    if (tab !== active) {
      stash();
      active = tab;
      showEditor();
      code.value = tab.value;
      code.setSelectionRange(tab.start, tab.end);
      code.scrollTop = tab.top;
      code.scrollLeft = tab.left;
    }
    showEditor();
    drawTabs();
    drawGutter(true);
    drawMark();
    code.focus();
  }

  // The lowest `untitled.m`, `untitled2.m`, ... no open tab is named.
  function untitledName() {
    for (let n = 1; ; n += 1) {
      const name = n === 1 ? 'untitled.m' : 'untitled' + n + '.m';
      const taken = tabs.some(function (tab) {
        return tab.name === name;
      });
      if (!taken) {
        return name;
      }
    }
  }

  function newTab() {
    const tab = makeTab('', untitledName(), '', false);
    tabs.push(tab);
    activate(tab);
  }

  function tabAt(path) {
    return (
      tabs.find(function (tab) {
        return tab.path !== '' && tab.path === path;
      }) || null
    );
  }

  // Opens the file at `path`, relative to the file root, in a tab of its
  // own, or selects its tab when it is open already; with a line, puts the
  // cursor at the start of it.
  function openFile(path, line) {
    const open = tabAt(path);
    if (open !== null) {
      activate(open);
      if (line) {
        goToLine(line, false);
      }
      return Promise.resolve();
    }
    return call({ op: 'read_file', path: path })
      .then(function (answer) {
        if (!answer.ok) {
          say(answer.error.message);
          return;
        }
        say('');
        let tab = tabAt(answer.path);
        if (tab === null) {
          const crlf = answer.text.indexOf('\r\n') !== -1;
          // The editor holds LF line ends only, as a textarea does.
          const text = answer.text.replace(/\r\n?/g, '\n');
          tab = makeTab(answer.path, lastName(answer.path), text, crlf);
          tabs.push(tab);
        }
        activate(tab);
        if (line) {
          goToLine(line, false);
        }
      })
      .catch(function (e) {
        say(e.message);
      });
  }

  // Puts the cursor at the start of line `line` of the active tab and
  // scrolls it into view; with `mark`, marks it in the gutter until the
  // next edit.
  function goToLine(line, mark) {
    if (active === null) {
      return;
    }
    const text = code.value;
    const last = Math.max(1, Math.min(line, lineCount(text)));
    let at = 0;
    for (let k = 1; k < last; k += 1) {
      at = text.indexOf('\n', at) + 1;
    }
    code.focus();
    code.setSelectionRange(at, at);
    const height = lineHeight();
    code.scrollTop = Math.max(0, (last - 1) * height - code.clientHeight / 2 + height);
    code.scrollLeft = 0;
    active.mark = mark ? last : active.mark;
    drawGutter(false);
    drawMark();
  }

  // After every change of the text: the mark goes, the gutter keeps step,
  // and the tab's • follows whether the text differs from the file.
  function edited() {
    if (active === null) {
      return;
    }
    if (active.mark > 0) {
      active.mark = 0;
      drawMark();
    }
    drawGutter(false);
    if (isDirty(active) !== active.shownDirty) {
      drawTabs();
    }
  }

  // Replaces the text from `start` to `end` with `text` through the
  // browser's own editing where it has it, so that its undo still works,
  // and selects from `from` to `to` afterwards.
  function replaceRange(start, end, text, from, to) {
    code.focus();
    code.setSelectionRange(start, end);
    let done = false;
    try {
      done = document.execCommand('insertText', false, text);
    } catch (e) {
      done = false;
    }
    if (!done) {
      code.setRangeText(text, start, end, 'end');
    }
    code.setSelectionRange(from, to);
    edited();
  }

  // The whole lines the selection touches, or the cursor's line: a
  // selection that ends at the start of a line does not take that line.
  function selectedLines() {
    const text = code.value;
    const start = code.selectionStart;
    const end = code.selectionEnd;
    const first = lineStart(text, start);
    const stop = end > start && text[end - 1] === '\n' ? end - 1 : end;
    let last = text.indexOf('\n', stop);
    if (last === -1) {
      last = text.length;
    }
    return { text: text, start: start, end: end, first: first, last: last };
  }

  // Tab: four spaces at the cursor, or before each line of a selection
  // that spans lines.
  function indent() {
    const s = selectedLines();
    if (s.text.slice(s.start, s.end).indexOf('\n') === -1) {
      const after = s.start + INDENT.length;
      replaceRange(s.start, s.end, INDENT, after, after);
      return;
    }
    const lines = s.text.slice(s.first, s.last).split('\n');
    const out = lines
      .map(function (line) {
        return INDENT + line;
      })
      .join('\n');
    replaceRange(s.first, s.last, out, s.first, s.first + out.length);
  }

  // Shift+Tab: up to four spaces taken from the start of each line the
  // selection touches.
  function outdent() {
    const s = selectedLines();
    const lines = s.text.slice(s.first, s.last).split('\n');
    let firstTaken = 0;
    let taken = 0;
    const out = lines
      .map(function (line, k) {
        let n = 0;
        while (n < INDENT.length && line[n] === ' ') {
          n += 1;
        }
        if (k === 0) {
          firstTaken = n;
        }
        taken += n;
        return line.slice(n);
      })
      .join('\n');
    if (taken === 0) {
      return;
    }
    const from = Math.max(s.first, s.start - firstTaken);
    const to = Math.max(from, s.end - taken);
    replaceRange(s.first, s.last, out, from, to);
  }

  // Writes the tab's text to `path`, with CRLF line ends put back when the
  // file had them. Resolves to true once the file holds the text.
  function writeTab(tab, path) {
    const value = textOf(tab);
    const text = tab.crlf ? value.replace(/\n/g, '\r\n') : value;
    return call({ op: 'write_file', path: path, text: text })
      .then(function (answer) {
        if (!answer.ok) {
          say(answer.error.message);
          return false;
        }
        say('');
        tab.saved = value;
        tab.path = answer.path;
        tab.name = lastName(answer.path);
        drawTabs();
        refreshFiles();
        return true;
      })
      .catch(function (e) {
        say(e.message);
        return false;
      });
  }

  // Asks in the page for the path a tab that was never saved is saved at,
  // and saves it there. Resolves to true once it is saved, and to false
  // when the question is cancelled.
  function askPath(tab) {
    if (pathAsked !== null) {
      pathAsked.resolve(false);
    }
    return new Promise(function (resolve) {
      pathAsked = { tab: tab, resolve: resolve };
      saveAs.hidden = false;
      savePath.value = joined(folder, tab.name);
      savePath.focus();
      const at = savePath.value.lastIndexOf('/') + 1;
      const dot = savePath.value.lastIndexOf('.');
      savePath.setSelectionRange(at, dot > at ? dot : savePath.value.length);
    });
  }

  function answerPath(ok) {
    const asked = pathAsked;
    if (asked === null) {
      return;
    }
    if (!ok) {
      pathAsked = null;
      saveAs.hidden = true;
      say('');
      asked.resolve(false);
      code.focus();
      return;
    }
    writeTab(asked.tab, savePath.value).then(function (done) {
      // A refusal leaves the question open, its reason shown, to be
      // answered again or cancelled.
      if (done && pathAsked === asked) {
        pathAsked = null;
        saveAs.hidden = true;
        asked.resolve(true);
        if (asked.tab === active) {
          code.focus();
        }
      }
    });
  }

  // Saves the tab: to its file, or, never saved, where the user names.
  function save(tab) {
    if (tab.path === '') {
      if (tab !== active) {
        activate(tab);
      }
      return askPath(tab);
    }
    return writeTab(tab, tab.path);
  }

  function closeTab(tab) {
    const at = tabs.indexOf(tab);
    if (at === -1) {
      return;
    }
    tabs.splice(at, 1);
    if (pathAsked !== null && pathAsked.tab === tab) {
      answerPath(false);
    }
    if (tab !== active) {
      drawTabs();
      return;
    }
    active = null;
    if (tabs.length > 0) {
      activate(tabs[Math.min(at, tabs.length - 1)]);
      return;
    }
    code.value = '';
    gutterCount = 0;
    gutterLines.textContent = '';
    drawMark();
    showEditor();
    drawTabs();
  }

  // Closing a tab with unsaved changes asks in the page first: Save,
  // Discard or Cancel.
  function askClose(tab) {
    if (!isDirty(tab)) {
      closeTab(tab);
      return;
    }
    activate(tab);
    closing = tab;
    closeText.textContent = tab.name + ' has changes that are not saved.';
    closeAsk.hidden = false;
    closeSave.focus();
  }

  function answerClose(choice) {
    const tab = closing;
    closing = null;
    closeAsk.hidden = true;
    if (tab === null) {
      return;
    }
    if (choice === 'save') {
      save(tab).then(function (done) {
        if (done) {
          closeTab(tab);
        }
      });
    } else if (choice === 'discard') {
      closeTab(tab);
    } else if (tab === active) {
      code.focus();
    }
  }

  // Run: saves the tab when it has changes or was never saved, then runs
  // its file, shown in the command window as run('<path>'). An error then
  // takes the cursor to its line: the innermost frame whose file is open
  // in a tab, or else the frame of the file that was run, the outermost.
  async function runFile() {
    const tab = active;
    if (tab === null || editorBusy) {
      return;
    }
    editorBusy = true;
    try {
      if (tab.path === '' || isDirty(tab)) {
        const done = await save(tab);
        if (!done) {
          return;
        }
      }
      const path = tab.path;
      const shown = "run('" + path.replace(/'/g, "''") + "')";
      const answer = await runEntry(shown, { op: 'run_file', path: path });
      if (answer !== null && !answer.ok) {
        showRunError(answer.error.stack || [], path);
      }
    } finally {
      editorBusy = false;
    }
  }

  function showRunError(frames, path) {
    let target = null;
    let line = null;
    for (let k = 0; k < frames.length && target === null; k += 1) {
      if (frames[k].file !== null) {
        target = tabAt(frames[k].file);
        line = frames[k].line;
      }
    }
    if (target === null && frames.length > 0) {
      target = tabAt(path);
      line = frames[frames.length - 1].line;
    }
    if (target === null) {
      return;
    }
    activate(target);
    if (line !== null) {
      goToLine(line, true);
    }
  }

  // Run Selection: the selection, or the cursor's line when nothing is
  // selected, as one entry typed as it stands; an error's line counts
  // from the selection's first line.
  async function runSelection() {
    const tab = active;
    if (tab === null || editorBusy) {
      return;
    }
    const text = code.value;
    let start = code.selectionStart;
    let end = code.selectionEnd;
    if (start === end) {
      start = lineStart(text, start);
      end = text.indexOf('\n', start);
      if (end === -1) {
        end = text.length;
      }
    }
    const chosen = text.slice(start, end);
    if (chosen.trim() === '') {
      return;
    }
    const firstLine = lineCount(text.slice(0, start));
    editorBusy = true;
    try {
      const answer = await run(chosen);
      if (answer !== null && !answer.ok && answer.error.line !== null && tabs.indexOf(tab) !== -1) {
        activate(tab);
        goToLine(firstLine + answer.error.line - 1, true);
      }
    } finally {
      editorBusy = false;
    }
  }

  code.addEventListener('input', edited);
  code.addEventListener('scroll', function () {
    gutter.scrollTop = code.scrollTop;
  });
  code.addEventListener('blur', function () {
    tabMoves = false;
  });
  code.addEventListener('keydown', function (ev) {
    if (ev.key === 'Escape') {
      tabMoves = true;
      return;
    }
    // A modifier pressed on its way to Shift+Tab is not the next key.
    if (ev.key === 'Shift' || ev.key === 'Control' || ev.key === 'Alt' || ev.key === 'Meta') {
      return;
    }
    if (ev.key === 'Tab' && !ev.ctrlKey && !ev.altKey && !ev.metaKey && !ev.isComposing) {
      if (tabMoves) {
        // The browser moves the focus on, so the keyboard can leave.
        tabMoves = false;
        return;
      }
      ev.preventDefault();
      if (ev.shiftKey) {
        outdent();
      } else {
        indent();
      }
      return;
    }
    tabMoves = false;
  });

  // The editor's keys work wherever the focus is inside the editor: Ctrl+S
  // (Cmd+S on a Mac) saves, F5 saves and runs, F9 runs the selection.
  editor.addEventListener('keydown', function (ev) {
    const command = ev.ctrlKey || ev.metaKey;
    if (command && !ev.altKey && !ev.shiftKey && (ev.key === 's' || ev.key === 'S')) {
      ev.preventDefault();
      if (pathAsked !== null) {
        answerPath(true);
      } else if (active !== null) {
        save(active);
      }
    } else if (ev.key === 'F5' && !command && !ev.altKey) {
      ev.preventDefault();
      runFile();
    } else if (ev.key === 'F9' && !command && !ev.altKey) {
      ev.preventDefault();
      runSelection();
    }
  });

  newButton.addEventListener('click', newTab);
  saveButton.addEventListener('click', function () {
    if (active !== null) {
      save(active);
    }
  });
  runButton.addEventListener('click', runFile);
  runSelectionButton.addEventListener('click', runSelection);
  saveAsOk.addEventListener('click', function () {
    answerPath(true);
  });
  saveAsCancel.addEventListener('click', function () {
    answerPath(false);
  });
  savePath.addEventListener('keydown', function (ev) {
    if (ev.key === 'Enter' && !ev.isComposing) {
      ev.preventDefault();
      answerPath(true);
    } else if (ev.key === 'Escape') {
      ev.preventDefault();
      answerPath(false);
    }
  });
  closeSave.addEventListener('click', function () {
    answerClose('save');
  });
  closeDiscard.addEventListener('click', function () {
    answerClose('discard');
  });
  closeCancel.addEventListener('click', function () {
    answerClose('cancel');
  });
  closeAsk.addEventListener('keydown', function (ev) {
    if (ev.key === 'Escape') {
      ev.preventDefault();
      answerClose('cancel');
    }
  });

  // Leaving or reloading the page with any tab unsaved: the browser's own
  // warning.
  window.addEventListener('beforeunload', function (ev) {
    if (tabs.some(isDirty)) {
      ev.preventDefault();
      ev.returnValue = '';
    }
  });

  // ---- The command history -----------------------------------------------

  // An entry of the history pane: a click puts it in the input, a double
  // click runs it.
  function showHistoryEntry(entry, index) {
    const li = element('li');
    const button = element('button', 'name', entry);
    button.type = 'button';
    button.title = entry;
    button.addEventListener('click', function () {
      if (!busy) {
        place = index;
        draft = '';
        setInput(entry);
        input.focus();
      }
    });
    button.addEventListener('dblclick', function () {
      if (!busy) {
        setInput(entry);
        enter();
      }
    });
    li.appendChild(button);
    historyList.appendChild(li);
    historyBody.scrollTop = historyBody.scrollHeight;
  }

  function addHistory(entry) {
    history.push(entry);
    showHistoryEntry(entry, history.length - 1);
    place = history.length;
    draft = '';
  }

  let shownHistoryNote = '';

  function historyNote(text) {
    if (text !== shownHistoryNote) {
      shownHistoryNote = text;
      historyList.appendChild(element('li', 'note', text));
    }
  }

  function loadHistory() {
    return call({ op: 'history' })
      .then(function (answer) {
        if (answer.ok) {
          answer.items.forEach(function (entry) {
            addHistory(entry);
          });
        }
      })
      .catch(function () {});
  }

  // ---- The splitters -----------------------------------------------------

  // No pane is made smaller than this, and a focused splitter moves this
  // far for each arrow key.
  const MIN = 120;
  const STEP = 16;
  const narrow = window.matchMedia('(max-width: 719.98px)');

  // The file browser's width, the right-hand column's, the workspace's
  // height and the editor's, in CSS pixels. The last two are measured on
  // the first wide layout, and 0 until then.
  const sizes = { left: 240, right: 360, top: 0, editor: 0 };

  function columnRoom() {
    return desktop.clientWidth - splitLeft.offsetWidth - splitRight.offsetWidth;
  }

  function sideRoom() {
    return side.clientHeight - splitSide.offsetHeight;
  }

  function middleRoom() {
    return middle.clientHeight - splitMid.offsetHeight;
  }

  function highest(which) {
    if (which === 'left') {
      return columnRoom() - sizes.right - MIN;
    }
    if (which === 'right') {
      return columnRoom() - sizes.left - MIN;
    }
    if (which === 'editor') {
      return middleRoom() - MIN;
    }
    return sideRoom() - MIN;
  }

  function splitterOf(which) {
    if (which === 'left') {
      return splitLeft;
    }
    if (which === 'editor') {
      return splitMid;
    }
    return which === 'right' ? splitRight : splitSide;
  }

  // Sets one size within its bounds and lays the panes out again. Only a
  // wide layout reads these sizes; a narrow one stacks the panes.
  function setSize(which, value) {
    if (narrow.matches) {
      return;
    }
    const high = highest(which);
    sizes[which] = Math.round(Math.max(MIN, Math.min(high, value)));
    desktop.style.setProperty('--left', sizes.left + 'px');
    desktop.style.setProperty('--right', sizes.right + 'px');
    if (sizes.top > 0) {
      side.style.setProperty('--top', sizes.top + 'px');
    }
    if (sizes.editor > 0) {
      middle.style.setProperty('--editor', sizes.editor + 'px');
    }
    const splitter = splitterOf(which);
    splitter.setAttribute('aria-valuemin', String(MIN));
    splitter.setAttribute('aria-valuemax', String(Math.max(MIN, high)));
    splitter.setAttribute('aria-valuenow', String(sizes[which]));
  }

  function layout() {
    if (narrow.matches) {
      return;
    }
    if (sizes.top === 0) {
      sizes.top = Math.max(MIN, Math.round(sideRoom() / 2));
    }
    if (sizes.editor === 0) {
      sizes.editor = Math.max(MIN, Math.round(middleRoom() / 2));
    }
    setSize('left', sizes.left);
    setSize('right', sizes.right);
    setSize('top', sizes.top);
    setSize('editor', sizes.editor);
  }

  // `sign` is +1 when moving the splitter right or down grows the size,
  // and -1 when it shrinks it.
  function splitter(el, which, horizontal, sign) {
    el.addEventListener('pointerdown', function (ev) {
      if (ev.button !== 0 || narrow.matches) {
        return;
      }
      ev.preventDefault();
      el.focus();
      el.setPointerCapture(ev.pointerId);
      el.classList.add('dragging');
      const start = horizontal ? ev.clientY : ev.clientX;
      const from = sizes[which];
      function move(e) {
        const at = horizontal ? e.clientY : e.clientX;
        setSize(which, from + sign * (at - start));
      }
      function end(e) {
        if (el.hasPointerCapture(e.pointerId)) {
          el.releasePointerCapture(e.pointerId);
        }
        el.classList.remove('dragging');
        el.removeEventListener('pointermove', move);
        el.removeEventListener('pointerup', end);
        el.removeEventListener('pointercancel', end);
      }
      el.addEventListener('pointermove', move);
      el.addEventListener('pointerup', end);
      el.addEventListener('pointercancel', end);
    });
    el.addEventListener('keydown', function (ev) {
      let step = 0;
      switch (ev.key) {
        case 'ArrowLeft':
          step = horizontal ? 0 : -1;
          break;
        case 'ArrowRight':
          step = horizontal ? 0 : 1;
          break;
        case 'ArrowUp':
          step = horizontal ? -1 : 0;
          break;
        case 'ArrowDown':
          step = horizontal ? 1 : 0;
          break;
        default:
          break;
      }
      if (step !== 0) {
        ev.preventDefault();
        setSize(which, sizes[which] + sign * step * STEP);
      }
    });
  }

  splitter(splitLeft, 'left', false, 1);
  splitter(splitRight, 'right', false, -1);
  splitter(splitSide, 'top', true, 1);
  splitter(splitMid, 'editor', true, 1);
  window.addEventListener('resize', layout);
  layout();

  // ---- Start -------------------------------------------------------------

  if (token === '') {
    note('No session token: open the address that splatcrab --ui printed.');
  }
  showEditor();
  drawTools();
  loadHistory();
  refreshWorkspace();
  refreshFiles();
  input.focus();
})();
