// SplatCrab desktop.
//
// Four panes over the evaluation protocol (docs/modules/U0-ui-foundations.md
// and docs/modules/U2-ui-desktop.md): the command window in the middle, the
// file browser on the left, and the workspace above the command history on
// the right, with a splitter between each pair.
//
// Every request is one POST /api carrying the session token from this
// page's URL fragment in the X-SplatCrab-Token header, and every request
// goes through one queue, one at a time in the order the page made them, so
// the panes' refreshes never race the command window or each other.
//
// Everything a request returns reaches the page as text (textContent and
// createTextNode), never as markup: variable names, previews, file names
// and history entries are the user's own. The page's
// Content-Security-Policy forbids inline script and style, which is why
// this file is separate and why pane sizes are custom properties set
// through the CSS object model rather than style attributes.

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

  // Runs the entry: its input, then its output exactly as the interpreter
  // wrote it, then its error, if any, set apart. The entry goes to the
  // shared history as it is run, and the workspace and the file browser
  // are refreshed afterwards, whatever the evaluation answered.
  async function run(code) {
    const entry = element('div', 'entry');
    entry.appendChild(block('in', code));
    transcript.appendChild(entry);
    scrollToEnd();
    call({ op: 'history_add', entry: code })
      .then(function (answer) {
        if (answer.ok && answer.added) {
          addHistory(code);
        } else if (!answer.ok) {
          historyNote(answer.error.message);
        }
      })
      .catch(function () {});
    try {
      const answer = await call({ op: 'eval', code: code });
      if (answer.out) {
        entry.appendChild(block('out', answer.out));
      }
      if (!answer.ok) {
        entry.appendChild(block('err', 'Error: ' + answer.error.message));
      }
    } catch (e) {
      entry.appendChild(block('err', e.message));
    }
    scrollToEnd();
    refreshWorkspace();
    refreshFiles();
  }

  async function enter() {
    const code = input.value;
    if (busy || code.trim() === '') {
      return;
    }
    busy = true;
    input.readOnly = true;
    try {
      const answer = await call({ op: 'complete', code: code });
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
      await run(code);
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

  function fileItem(name, size) {
    const li = element('li');
    const label = element('span', 'name', name);
    label.title = name;
    li.appendChild(label);
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
      filesList.appendChild(
        e.dir ? folderItem(e.name, joined(answer.path, e.name), false) : fileItem(e.name, e.size)
      );
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

  // The file browser's width, the right-hand column's, and the
  // workspace's height, in CSS pixels. The last is measured on the first
  // wide layout, and 0 until then.
  const sizes = { left: 240, right: 360, top: 0 };

  function columnRoom() {
    return desktop.clientWidth - splitLeft.offsetWidth - splitRight.offsetWidth;
  }

  function sideRoom() {
    return side.clientHeight - splitSide.offsetHeight;
  }

  function highest(which) {
    if (which === 'left') {
      return columnRoom() - sizes.right - MIN;
    }
    if (which === 'right') {
      return columnRoom() - sizes.left - MIN;
    }
    return sideRoom() - MIN;
  }

  function splitterOf(which) {
    if (which === 'left') {
      return splitLeft;
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
    setSize('left', sizes.left);
    setSize('right', sizes.right);
    setSize('top', sizes.top);
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
  window.addEventListener('resize', layout);
  layout();

  // ---- Start -------------------------------------------------------------

  if (token === '') {
    note('No session token: open the address that splatcrab --ui printed.');
  }
  loadHistory();
  refreshWorkspace();
  refreshFiles();
  input.focus();
})();
