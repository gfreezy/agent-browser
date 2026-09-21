// No website access, login state access, messaging endpoint, or debugger attachment.
// debugger.getTargets only maps the CDP target ID to Chrome's numeric tab ID.
globalThis.xfpWindow = {
  async ensureWindow() {
    const windows = await chrome.windows.getAll({ windowTypes: ["normal"] });
    if (windows.length) return windows[0].id;
    return (await chrome.windows.create({ url: "about:blank", focused: false })).id;
  },
  async select(targetId) {
    const target = (await chrome.debugger.getTargets()).find(t => t.id === targetId);
    if (!Number.isInteger(target?.tabId)) throw new Error("Collection tab no longer exists");
    const tab = await chrome.tabs.update(target.tabId, { active: true });
    return { tabId: tab.id, windowId: tab.windowId };
  }
};
// Ensure installation starts the worker, without automatically creating windows on profile import.
chrome.runtime.onInstalled.addListener(() => {});
