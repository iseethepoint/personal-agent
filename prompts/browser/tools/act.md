Perform a short sequence of actions on a tab, such as clicking, typing, choosing an option, scrolling or navigating.

State the intent of the actions briefly. Use `ref` when the referenced element is itself the target, such as a button or text field.

Choosing an option and ticking a box take element references. A wait requires `condition: "visible"` or `condition: "hidden"` and exactly one of `ref` or `text`, naming what should appear or disappear. Typing takes the field's reference too. A key press takes the element's reference, or no target to press the key wherever the focus is. A scroll takes the reference of the area to scroll inside, or no target to scroll the page. Include only steps whose targets are already known, and stop where the next step depends on what happens.

A wait ends as soon as what it names appears or disappears, or after `timeout_ms`, which is 5 seconds when left out. A wait for a change that never comes lasts its whole timeout, so leave `timeout_ms` out unless the page shows the change takes longer, such as a processing message or a check that verifies the browser by itself. A wait that times out reports `condition_not_met`, and the actions after it are not sent. Read the returned view before waiting again; the page may show an error or a different result instead.

Most fills leave out `entry`, and the browser enters the text its usual way. `typing` presses a key for each character, for a field that reacts to each key. `paste` pastes the whole text at once, for a field that expects pasted text. With either, `replace` still puts the text in place of the field's text and `append` still adds it after.

The result reports each action and includes the updated view with a current screenshot. The tab's older views can then lose their page content and screenshot. When an action opens a new tab, the result names the tab and shows its view. The sequence stops early when the page changes, a target is refused or an outcome is uncertain. Do not repeat an action whose outcome is uncertain.

A refused step says why. `covered` means something lies over the target, so close it or reach the target another way. `not_visible` means the target is off-screen or hidden, so scroll or open what holds it. `disabled` means the page waits for something first. `page_unresponsive` means the page did not answer in time and the action may still have run, so observe with `browser.observe` before repeating it. `needs_reference` means this browser can do that step only through the element's reference, such as choosing an option, ticking a box or scrolling inside an area. If the reference was refused for a reason you can fix, such as something covering it, fix that and use the reference from the current view. If the target has no reference, this browser cannot do that there: reach the goal another way the site offers, or say so in `browser.report`.

This tool cannot enter secrets, upload or download files, or save pages as PDFs.
