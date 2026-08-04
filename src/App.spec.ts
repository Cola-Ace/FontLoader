import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App.vue";
import type { AppSnapshot, BatchProgress, BatchResult } from "./types";

const { invoke, listen, open } = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  open: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open }));

const emptySnapshot = (): AppSnapshot => ({
  fonts: [],
  restoreCandidates: [],
  exitErrors: [],
});

const font = {
  canonicalPath: "C:\\Fonts\\Example.ttf",
  fileName: "Example.ttf",
  format: "TTF" as const,
  faces: [{ family: "Example Sans", style: "Regular" }],
  loadedAt: "2026-08-02T12:00:00Z",
};

function mockInitial(snapshot: AppSnapshot) {
  invoke.mockImplementation((command: string) => {
    if (command === "get_initial_state") {
      return Promise.resolve(snapshot);
    }
    return Promise.resolve(undefined);
  });
}

describe("App", () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockReset();
    listen.mockResolvedValue(() => {});
    open.mockReset();
    localStorage.clear();
  });

  it("switches and persists the selected theme", async () => {
    mockInitial(emptySnapshot());
    const wrapper = mount(App);
    await flushPromises();

    await wrapper.get('button[aria-label="切换到深色主题"]').trigger("click");

    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(localStorage.getItem("font-loader-theme")).toBe("dark");
    expect(wrapper.get('button[aria-label="切换到浅色主题"]')).toBeTruthy();
  });

  it("renders the empty state", async () => {
    mockInitial(emptySnapshot());
    const wrapper = mount(App);
    await flushPromises();

    expect(wrapper.text()).toContain("尚未加载字体");
    expect(invoke).toHaveBeenCalledWith("window_ready");
  });

  it("shows the last-session restore action", async () => {
    mockInitial({
      ...emptySnapshot(),
      restoreCandidates: ["C:\\Fonts\\Example.ttf", "C:\\Fonts\\Other.otf"],
    });
    const wrapper = mount(App);
    await flushPromises();

    expect(wrapper.text()).toContain("2 个字体可恢复");
    expect(wrapper.text()).toContain("恢复");
  });

  it("sorts fonts by name and loaded time in both directions", async () => {
    const older = {
      ...font,
      canonicalPath: "C:\\Fonts\\Zulu.ttf",
      fileName: "Zulu.ttf",
      faces: [{ family: "Zulu Sans", style: "Regular" }],
      loadedAt: "2026-08-01T12:00:00Z",
    };
    const newer = {
      ...font,
      canonicalPath: "C:\\Fonts\\Alpha.ttf",
      fileName: "Alpha.ttf",
      faces: [{ family: "Alpha Sans", style: "Regular" }],
      loadedAt: "2026-08-03T12:00:00Z",
    };
    mockInitial({ ...emptySnapshot(), fonts: [older, newer] });
    const wrapper = mount(App);
    await flushPromises();

    const names = () =>
      wrapper
        .findAll("[data-testid='font-row']")
        .map(row => row.find(".font-identity strong").text());

    expect(names()).toEqual(["Alpha Sans · Regular", "Zulu Sans · Regular"]);

    await wrapper.get('[data-testid="font-sort"]').setValue("name-desc");
    expect(names()).toEqual(["Zulu Sans · Regular", "Alpha Sans · Regular"]);

    await wrapper.get('[data-testid="font-sort"]').setValue("loaded-at-asc");
    expect(names()).toEqual(["Zulu Sans · Regular", "Alpha Sans · Regular"]);

    await wrapper.get('[data-testid="font-sort"]').setValue("loaded-at-desc");
    expect(names()).toEqual(["Alpha Sans · Regular", "Zulu Sans · Regular"]);
  });

  it("loads all files returned by the picker", async () => {
    mockInitial(emptySnapshot());
    open.mockResolvedValue(["C:\\Fonts\\A.ttf", "C:\\Fonts\\B.otf"]);
    const result: BatchResult = {
      snapshot: { ...emptySnapshot(), fonts: [font] },
      changed: [font.canonicalPath],
      skipped: [],
      errors: [],
    };
    invoke.mockImplementation((command: string) => {
      if (command === "get_initial_state") {
        return Promise.resolve(emptySnapshot());
      }
      if (command === "load_fonts") {
        return Promise.resolve(result);
      }
      return Promise.resolve(undefined);
    });
    const wrapper = mount(App);
    await flushPromises();

    await wrapper.get(".button-primary").trigger("click");
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("load_fonts", {
      paths: ["C:\\Fonts\\A.ttf", "C:\\Fonts\\B.otf"],
    });
    expect(wrapper.findAll("[data-testid='font-row']")).toHaveLength(1);
  });

  it("shows progress while loading files", async () => {
    mockInitial(emptySnapshot());
    open.mockResolvedValue(["C:\\Fonts\\A.ttf", "C:\\Fonts\\B.otf"]);
    let resolveBatch!: (result: BatchResult) => void;
    const pendingBatch = new Promise<BatchResult>(resolve => {
      resolveBatch = resolve;
    });
    let progressHandler!: (event: { payload: BatchProgress }) => void;
    listen.mockImplementation(async (_event, handler) => {
      progressHandler = handler as (event: { payload: BatchProgress }) => void;
      return () => {};
    });
    invoke.mockImplementation((command: string) => {
      if (command === "get_initial_state") {
        return Promise.resolve(emptySnapshot());
      }
      if (command === "load_fonts") {
        return pendingBatch;
      }
      return Promise.resolve(undefined);
    });
    const wrapper = mount(App);
    await flushPromises();

    await wrapper.get(".button-primary").trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("0/2");
    progressHandler({ payload: { operation: "load", completed: 1, total: 2 } });
    await flushPromises();
    expect(wrapper.text()).toContain("1/2");

    resolveBatch({
      snapshot: { ...emptySnapshot(), fonts: [font] },
      changed: [font.canonicalPath],
      skipped: [],
      errors: [],
    });
    await flushPromises();
    expect(wrapper.text()).toContain("加载成功 1 个");
  });

  it("dismisses a failed operation notice automatically", async () => {
    vi.useFakeTimers();
    try {
      mockInitial(emptySnapshot());
      open.mockResolvedValue(["C:\\Fonts\\A.ttf"]);
      invoke.mockImplementation((command: string) => {
        if (command === "get_initial_state") {
          return Promise.resolve(emptySnapshot());
        }
        if (command === "load_fonts") {
          return Promise.reject(new Error("加载失败"));
        }
        return Promise.resolve(undefined);
      });
      const wrapper = mount(App);
      await flushPromises();

      await wrapper.get(".button-primary").trigger("click");
      await flushPromises();

      expect(wrapper.text()).toContain("加载失败");
      vi.advanceTimersByTime(3000);
      await flushPromises();
      expect(wrapper.find(".operation-status").exists()).toBe(false);
      wrapper.unmount();
    } finally {
      vi.useRealTimers();
    }
  });

  it("unloads checked rows in one batch", async () => {
    mockInitial({ ...emptySnapshot(), fonts: [font] });
    const wrapper = mount(App);
    await flushPromises();

    await wrapper.get(`input[aria-label="选择 ${font.fileName}"]`).setValue(true);
    invoke.mockResolvedValue({
      snapshot: emptySnapshot(),
      changed: [font.canonicalPath],
      skipped: [],
      errors: [],
    });
    await wrapper.get(".button-danger-quiet").trigger("click");
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("unload_fonts", {
      paths: [font.canonicalPath],
    });
  });

  it("shows exit failures and supports force exit", async () => {
    mockInitial({
      ...emptySnapshot(),
      exitErrors: [
        {
          path: font.canonicalPath,
          code: "system_unload_failed",
          message: "RemoveFontResourceExW 返回 0",
        },
      ],
    });
    const wrapper = mount(App);
    await flushPromises();

    expect(wrapper.get("[role='dialog']").text()).toContain("部分字体卸载失败");
    invoke.mockResolvedValue({ exiting: true, errors: [] });
    await wrapper.get(".dialog-actions .button-secondary").trigger("click");
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("request_exit", { force: true });
  });
});
