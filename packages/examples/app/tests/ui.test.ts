import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("signed-out status is shown only by the header badge", async () => {
  const source = await readFile("src/App.tsx", "utf8");

  assert.match(source, /const showStatus = status !== "Signed out" && status !== "Signed in";/);
  assert.match(source, /showStatus \? <p className="status">\{status\}<\/p> : null/);
});

test("React shell does not render the Irongate logo", async () => {
  const source = await readFile("src/App.tsx", "utf8");

  assert.doesNotMatch(source, /irongateLogoUrl/);
  assert.doesNotMatch(source, /className="brand-mark"/);
});

test("Tauri app keeps the Irongate logo in the native icon folder", async () => {
  const logo = await readFile("src-tauri/icons/irongate-desktop-logo.png");

  assert.equal(logo.subarray(1, 4).toString("ascii"), "PNG");
});

test("Tauri startup wires the Irongate tray icon", async () => {
  const source = await readFile("src-tauri/src/lib.rs", "utf8");

  assert.match(source, /include_image!\("\.\/icons\/irongate-desktop-logo\.png"\)/);
  assert.match(source, /TrayIconBuilder::new\(\)/);
});
