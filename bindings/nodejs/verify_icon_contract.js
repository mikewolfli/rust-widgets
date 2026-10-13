// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//
// Node-language runtime verification of the icon_source contract through the real C ABI.
//
// Why this file exists
// --------------------
// The Rust tests exercise the property contract in-process, and the Python binding checks the ABI's
// own shape. Neither runs the *Node* binding, so the JS descriptor encoder/decoder
// (`builtin|name`, `resource|mode|id`) had never been executed against the library. A JS-side
// encoding bug would be invisible to both suites. This probe drives the same round trip the Rust
// tests assert, through the binding's koffi layer, so the JS half is verified by execution rather
// than by inspection.
//
// Runs on any Node >= 16 (measured 2026-10-13: 16.20.2, 18.20.8, 20.20.2, 22.23.3 and 24.15.0 each
// pass all 17 checks). The binding used to load `ffi-napi`, which could not be built on Node 18+ at
// all, so this probe was once restricted to a Node 16 toolchain; that constraint went away with the
// dependency. See `README.md` in this directory for the measured reasons.
//
// Usage (from this directory, after `npm install`):
//   cargo build --no-default-features --features desktop          # produces the cdylib
//   RUST_WIDGETS_LIB_PATH=../../target/debug/librust_widgets.so node verify_icon_contract.js

const path = require('path');

const { RustWidgets: RW, RW_VALUE_ICON_SPEC: KIND } = require(path.join(__dirname, 'index.js'));

let failures = 0;
function check(name, condition, detail) {
  if (condition) {
    console.log(`  ok   ${name}`);
  } else {
    failures += 1;
    console.log(`  FAIL ${name}${detail ? ` — ${detail}` : ''}`);
  }
}

const rw = RW.getInstance();
rw.init();

// A control needs a window to live in: `createWidgetOfKind(0, ...)` returns 0, which is the
// factory's honest "no control was made" rather than a usable orphan.
const windowId = rw.createWindow('Icon probe', 0, 0, 320, 240);
check('the window is created', windowId !== 0, `id=${windowId}`);

const button = rw.createWidgetOfKind(windowId, 'button', 'Save', 0, 0, 120, 32);
check('the factory creates a button by name', button !== 0, `id=${button}`);

const names = rw.widgetPropertyNames(button);
check(
  'the button publishes `icon_source`',
  Array.isArray(names) && names.includes('icon_source'),
  JSON.stringify(names && names.slice(0, 8)),
);

// 1. A built-in descriptor survives encode -> ABI -> decode, field for field.
const builtin = { kind: 'builtin', name: 'save' };
check('writing a built-in descriptor succeeds', rw.setWidgetProperty(button, 'icon_source', builtin) === true);
const readBack = rw.getWidgetProperty(button, 'icon_source');
check(
  'the built-in descriptor round-trips through the JS wire form',
  readBack && readBack.kind === 'builtin' && readBack.name === 'save',
  JSON.stringify(readBack),
);

// 2. An explicit clear is `null`, and it is distinguishable from "never set".
check('an explicit clear is accepted', rw.setWidgetProperty(button, 'icon_source', null) === true);
check('a cleared icon reads back as null', rw.getWidgetProperty(button, 'icon_source') === null);

// 3. A resource descriptor needs the host's resolver, so a plain property write must be refused —
//    not silently accepted as a path. If this ever returns true, the JS layer is claiming a
//    capability the library does not have.
const resource = { kind: 'resource', id: 'project/icons/save', color_mode: 'monochrome' };
const resourceWritten = rw.setWidgetProperty(button, 'icon_source', resource);
check(
  'a resource descriptor is refused without an apply context',
  resourceWritten === false,
  `setWidgetProperty returned ${resourceWritten}`,
);

// 4. The JS encoder refuses a malformed descriptor instead of putting a wrong wire form on the bus.
let encoderThrew = false;
try {
  rw.setWidgetProperty(button, 'icon_source', { kind: 'builtin' });
} catch (error) {
  encoderThrew = error instanceof TypeError;
}
check('a malformed descriptor is refused before it reaches the ABI', encoderThrew);

// 5. The wire constant the binding exports is the ABI's own value kind, so a host that branches on
//    it is branching on the real number rather than a copy that can drift.
check('the exported ICON_SPEC kind is 10', KIND === 10, `kind=${KIND}`);

// 6. The icon descriptor rides the same `(kind, num, text)` triple as every scalar property, so a
//    defect in a neighbouring branch would look exactly like an icon defect (and the first draft of
//    this probe only exercised the icon branch — a real gap, found by injecting a boolean bug that
//    the probe did not notice). Each remaining encoder branch is therefore driven once:
//    string, boolean, integer and float.
check(
  'a string property round-trips through the same encoder',
  rw.setWidgetProperty(button, 'text', 'Hello') === true &&
    rw.getWidgetProperty(button, 'text') === 'Hello',
  `read back ${JSON.stringify(rw.getWidgetProperty(button, 'text'))}`,
);
check(
  'a boolean property round-trips (the branch that produced a raw BigInt)',
  rw.setWidgetProperty(button, 'enabled', false) === true &&
    rw.getWidgetProperty(button, 'enabled') === false,
  `read back ${JSON.stringify(rw.getWidgetProperty(button, 'enabled'))}`,
);

// `progress_bar` publishes an integer property, and `rating` a **writable** float: the two numeric
// branches need different controls because `progress_bar.progress` is read-only (it is derived from
// `value`), and a read-only property is refused rather than silently ignored — which is what the
// first draft of this check measured, and why the write's own result is asserted separately below.
const bar = rw.createWidgetOfKind(windowId, 'progress_bar', '', 0, 60, 120, 16);
check('the factory creates a progress bar by name', bar !== 0n, `id=${bar}`);
check(
  'an integer property round-trips',
  rw.setWidgetProperty(bar, 'value', 7) === true && rw.getWidgetProperty(bar, 'value') === 7n,
  `read back ${rw.getWidgetProperty(bar, 'value')}`,
);

const rating = rw.createWidgetOfKind(windowId, 'rating', '', 0, 90, 120, 24);
check('the factory creates a rating by name', rating !== 0n, `id=${rating}`);
const floatAccepted = rw.setWidgetProperty(rating, 'value', 3.5);
check('a float property accepts a fractional value', floatAccepted === true, `write returned ${floatAccepted}`);
check(
  'a float property round-trips (its bit pattern exceeds 2^53 and must not be rounded)',
  rw.getWidgetProperty(rating, 'value') === 3.5,
  `wrote 3.5, read ${rw.getWidgetProperty(rating, 'value')}`,
);

console.log(failures === 0 ? '\nNODE-ICON-PROBE: PASS' : `\nNODE-ICON-PROBE: FAIL (${failures})`);
process.exit(failures === 0 ? 0 : 1);
