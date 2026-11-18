import "@testing-library/jest-dom/vitest";

if (!HTMLElement.prototype.scrollTo) {
  HTMLElement.prototype.scrollTo = () => undefined;
}
if (!HTMLElement.prototype.scrollIntoView) {
  HTMLElement.prototype.scrollIntoView = () => undefined;
}
// jsdom defines scrollTo but throws "not implemented"; the app only needs it
// to be a no-op during navigation in tests.
window.scrollTo = () => undefined;
