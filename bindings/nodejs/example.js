#!/usr/bin/env node

// ---------------------------------------------------------------------------
// Example usage of the rust-widgets Node.js binding
//
// This example demonstrates:
//   1. Loading the library and initializing the GUI
//   2. Creating a window with various widgets
//   3. Polling events in a simple loop
//   4. Cleaning up
//
// NOTE: rust-widgets requires a running event loop. The `run()` method is
// blocking, so in a real application you would call it on a dedicated thread
// or use the embedded engine mode.
//
// This example uses a timeout-based polling loop to demonstrate the API
// without blocking indefinitely.
// ---------------------------------------------------------------------------

const { RustWidgets, TriggerKind, PlatformCapability } = require('./index');

// ---------------------------------------------------------------------------
// Simple synchronous demo using the embedded engine pattern
// ---------------------------------------------------------------------------
function main() {
  console.log('=== rust-widgets Node.js Binding Example ===\n');

  // Get the singleton instance (loads the native library)
  const rw = RustWidgets.getInstance();

  // ── Query platform info before init ──────────────────────────────
  try {
    const apiVersion = rw.bindingsApiVersion();
    console.log('  Bindings API version:', apiVersion);
  } catch (e) {
    console.log('  (bindingsApiVersion not available — will query after init)');
  }

  // ── Initialize the GUI ───────────────────────────────────────────
  console.log('\n  Initializing rust-widgets...');
  rw.init();
  console.log('  ✅ Initialized');

  // ── Query platform capabilities ──────────────────────────────────
  const caps = rw.platformCapabilities();
  console.log('  Platform capabilities mask:', caps);

  const capNames = [];
  if (caps & PlatformCapability.DpiScaling)      capNames.push('DPI Scaling');
  if (caps & PlatformCapability.Ime)             capNames.push('IME');
  if (caps & PlatformCapability.Accessibility)   capNames.push('Accessibility');
  if (caps & PlatformCapability.NativeMenu)      capNames.push('Native Menu');
  if (caps & PlatformCapability.TypedWidgetTrigger) capNames.push('Typed Widget Trigger');
  console.log('  Available:', capNames.join(', ') || 'none reported');

  const backend = rw.backendName();
  console.log('  Backend:', backend);

  const dpiScale = rw.platformDpiScaleFactor();
  console.log('  DPI scale factor:', dpiScale);

  // ── Create a window ──────────────────────────────────────────────
  console.log('\n  Creating window...');
  const windowId = rw.createWindow('Node.js Demo', 100, 100, 640, 480);
  if (windowId === 0) {
    console.error('  ❌ Failed to create window');
    rw.quit();
    process.exit(1);
  }
  console.log('  ✅ Window created (ID:', windowId, ')');

  // ── Add widgets ──────────────────────────────────────────────────
  console.log('\n  Creating widgets...');

  const buttonId = rw.createButton(windowId, 'Click Me', 20, 40, 120, 32);
  console.log('  Button (ID:', buttonId, ')');

  const checkboxId = rw.createCheckbox(windowId, 'Enable Feature', 20, 80, 180, 28);
  console.log('  Checkbox (ID:', checkboxId, ')');

  const labelId = rw.createLabel(windowId, 'Hello from Node.js!', 20, 120, 200, 24);
  console.log('  Label (ID:', labelId, ')');

  const lineEditId = rw.createLineEdit(windowId, 'Type here...', 20, 160, 200, 28);
  console.log('  Line Edit (ID:', lineEditId, ')');

  const comboBoxId = rw.createComboBox(windowId, 20, 200, 180, 28);
  console.log('  Combo Box (ID:', comboBoxId, ')');

  // Populate combo box
  rw.comboBoxAddItem(comboBoxId, 'Option A');
  rw.comboBoxAddItem(comboBoxId, 'Option B');
  rw.comboBoxAddItem(comboBoxId, 'Option C');
  rw.comboBoxSetCurrentIndex(comboBoxId, 0);
  console.log('  Combo Box items:', rw.comboBoxItemCount(comboBoxId));

  const sliderId = rw.createSlider(windowId, 240, 40, 200, 28);
  console.log('  Slider (ID:', sliderId, ')');

  const progressId = rw.createProgressBar(windowId, 240, 80, 200, 28);
  console.log('  Progress Bar (ID:', progressId, ')');

  const panelId = rw.createPanel(windowId, 240, 120, 200, 160);
  console.log('  Panel (ID:', panelId, ')');

  // ── Widget manipulation ──────────────────────────────────────────
  console.log('\n  Manipulating widgets...');

  rw.setWidgetText(labelId, 'Updated label text!');
  const labelText = rw.getWidgetText(labelId);
  console.log('  Label text:', labelText);

  rw.setWidgetEnabled(buttonId, true);
  const isEnabled = rw.isWidgetEnabled(buttonId);
  console.log('  Button enabled:', isEnabled);

  rw.setWidgetVisible(checkboxId, true);
  const isVisible = rw.isWidgetVisible(checkboxId);
  console.log('  Checkbox visible:', isVisible);

  // Set a combo box item
  rw.comboBoxSetCurrentIndex(comboBoxId, 2);
  const current = rw.comboBoxCurrentIndex(comboBoxId);
  const currentText = rw.comboBoxItemText(comboBoxId, current);
  console.log('  Combo Box current:', current, '->', currentText);

  // ── Show all widgets ─────────────────────────────────────────────
  console.log('\n  Showing widgets...');

  // Window is shown by default; explicitly show others
  rw.showWidget(buttonId);
  rw.showWidget(checkboxId);
  rw.showWidget(labelId);
  rw.showWidget(lineEditId);
  rw.showWidget(comboBoxId);
  rw.showWidget(sliderId);
  rw.showWidget(progressId);
  rw.showWidget(panelId);

  // ── Clipboard ────────────────────────────────────────────────────
  console.log('\n  Clipboard...');
  const clipOk = rw.setClipboardText('Hello from rust-widgets!');
  console.log('  Set clipboard:', clipOk ? '✅' : '❌');
  const clipText = rw.getClipboardText();
  console.log('  Clipboard text:', clipText);

  // ── Accessibility ────────────────────────────────────────────────
  console.log('\n  Accessibility...');
  const a11yOk = rw.setWidgetAccessibilityName(buttonId, 'Main Action Button');
  console.log('  Set a11y name:', a11yOk ? '✅' : '❌');
  const a11yName = rw.getWidgetAccessibilityName(buttonId);
  console.log('  A11y name:', a11yName);

  // ── Render settings ──────────────────────────────────────────────
  console.log('\n  Render settings...');
  const aaSamples = rw.getRenderAaSamplesPerAxis();
  console.log('  AA samples per axis:', aaSamples);

  // ── Embedded engine info ─────────────────────────────────────────
  console.log('\n  Embedded engine...');
  console.log('  Is initialized:', rw.embeddedEngineIsInitialized());
  console.log('  Is running:', rw.embeddedEngineIsRunning());

  // ── Poll for events (non-blocking, single pass) ──────────────────
  console.log('\n  Polling events (non-blocking)...');
  const triggeredId = rw.pollWidgetTriggered();
  if (triggeredId !== 0) {
    console.log('  Widget triggered (simple):', triggeredId);
  }

  const triggerEvent = rw.pollWidgetTriggerEvent();
  if (triggerEvent) {
    const kindName = Object.keys(TriggerKind).find(k => TriggerKind[k] === triggerEvent.kind) || 'Unknown';
    console.log('  Widget triggered (typed): ID=' + triggerEvent.widgetId + ', kind=' + kindName);
  }

  const menuId = rw.pollMenuTriggered();
  if (menuId !== 0) {
    console.log('  Menu triggered:', menuId);
  }

  // ── Drag & Drop ──────────────────────────────────────────────────
  console.log('\n  Drag & Drop...');
  const dragPayload = Buffer.from('drag payload data');
  const dragOk = rw.beginDrag(buttonId, 'text/plain', dragPayload);
  console.log('  Begin drag:', dragOk ? '✅' : '❌ (may not be supported)');

  const dropEvent = rw.pollDropEvent();
  if (dropEvent) {
    console.log('  Drop event: source=' + dropEvent.sourceWidgetId +
                ', target=' + dropEvent.targetWidgetId +
                ', mime=' + dropEvent.mimeType);
  } else {
    console.log('  No drop event pending');
  }

  // ── Error handling demo ──────────────────────────────────────────
  console.log('\n  Error handling...');
  try {
    // Using an invalid widget ID (0) should not crash
    rw.hideWidget(0);
    console.log('  hideWidget(0) did not crash');
  } catch (err) {
    console.log('  hideWidget(0) threw:', err.message);
  }

  // ── Check binding status ─────────────────────────────────────────
  console.log('\n  Binding status...');
  const nodeStatus = rw.nodejsBindingStatus();
  console.log('  Node.js binding status mask:', nodeStatus);

  try {
    const pyStatus = rw.pythonBindingStatus();
    console.log('  Python binding status mask:', pyStatus);
  } catch (_) { /* not available */ }

  try {
    const cppStatus = rw.cppBindingStatus();
    console.log('  C++ binding status mask:', cppStatus);
  } catch (_) { /* not available */ }

  // ── Cleanup ──────────────────────────────────────────────────────
  console.log('\n  Quitting...');
  rw.quit();
  console.log('  ✅ Quit signal sent');
  console.log('\n=== Example finished ===');
}

// Run
try {
  main();
} catch (err) {
  console.error('\n❌ Error:', err.message);
  console.error(err.stack);
  process.exit(1);
}
