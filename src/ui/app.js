// SplatCrab command window.
//
// Every entry goes to POST /api as one request of the evaluation protocol
// (docs/modules/U0-ui-foundations.md), with the session token from this
// page's URL fragment in the X-SplatCrab-Token header. Enter asks the server
// whether the entry is complete, the same question the terminal REPL asks,
// and runs it only when it is; otherwise it inserts a newline. Up and Down
// walk this page's history. The page's Content-Security-Policy forbids
// inline script and style, which is why this file is separate.

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
  const transcript = document.getElementById('transcript');
  const input = document.getElementById('input');

  const history = [];
  let place = 0; // index into history; history.length is the draft
  let draft = '';
  let nextId = 1;
  let busy = false;

  // One protocol request. A refusal by the server (403, 413, ...) is thrown
  // with the server's own status text as its message.
  async function call(request) {
    request.id = nextId++;
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

  function block(className, text) {
    const pre = document.createElement('pre');
    pre.className = className;
    pre.textContent = text;
    return pre;
  }

  function scrollToEnd() {
    transcript.scrollTop = transcript.scrollHeight;
  }

  function note(text) {
    const entry = document.createElement('div');
    entry.className = 'entry';
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

  function remember(code) {
    if (history[history.length - 1] !== code) {
      history.push(code);
    }
    place = history.length;
    draft = '';
  }

  // Runs the entry: its input, then its output exactly as the interpreter
  // wrote it, then its error, if any, set apart.
  async function run(code) {
    const entry = document.createElement('div');
    entry.className = 'entry';
    entry.appendChild(block('in', code));
    transcript.appendChild(entry);
    scrollToEnd();
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
      if (!answer.complete) {
        input.readOnly = false;
        insertNewline();
        return;
      }
      remember(code);
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

  if (token === '') {
    note('No session token: open the address that splatcrab --ui printed.');
  }
  input.focus();
})();
