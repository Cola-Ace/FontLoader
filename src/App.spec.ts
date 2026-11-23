import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AlertTriangle, CheckCircle2, CircleX } from "lucide-vue-next";
import App from "./App.vue";
import type {
  AppSnapshot,
  BatchCompletion,
  BatchProgress,
  BatchResult,
  InitialState,
} from "./types";

const { confirm, invoke, listen, open } = vi.hoisted(() => ({
  confirm: vi.fn(),
  invoke: vi.fn(),
  listen: vi.fn(),
  open: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm, open }));

const emptySnapshot = (): AppSnapshot => ({
  fonts: [],
  restoreCandidates: [],
  exitErrors: [],
});

const font = {
  canonicalPath: "C:\\Fonts\\Example.ttf",
  fileName: "Example.ttf",
  format: "TTF" as const,
  faces: [{ family: "Example Sans", familyAliases: [], style: "Regular" }],
  loadedAt: "2026-08-02T12:00:00Z",
};

const initialState = (snapshot: AppSnapshot, progress: BatchProgress | null = null): InitialState =>
  ({ snapshot, progress, completion: null });

const batchCompletion = (
  result: BatchResult,
  operation: BatchCompletion["operation"] = "load",
  id = 1,
): BatchCompletion => ({ id, operation, result });

function mockInitial(snapshot: AppSnapshot) {
  invoke.mockImplementation((command: string) => {
    if (command === "get_initial_state") {
      return Promise.resolve(initialState(snapshot));
    }
    return Promise.resolve(undefined);
  });
}

describe("App", () => {
  beforeEach(() => {
    confirm.mockReset();
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

  it("makes the window ready before waiting for the initial state", async () => {
    let resolveInitial!: (state: InitialState) => void;
    const pendingInitial = new Promise<InitialState>(resolve => {
      resolveInitial = resolve;
    });
    invoke.mockImplementation((command: string) => {
      if (command === "get_initial_state") {
        return pendingInitial;
      }
      return Promise.resolve(undefined);
    });

    const wrapper = mount(App);
    await flushPromises();

    expect(invoke.mock.calls.map(([command]) => command)).toEqual([
      "window_ready",
      "get_initial_state",
    ]);

    expect(wrapper.get(".button-primary").attributes("disabled")).toBeDefined();
    resolveInitial(initialState(emptySnapshot()));
    await flushPromises();
    wrapper.unmount();
  });

  it("restores in-flight progress and completion after reopening", async () => {
    let progressHandler!: (event: { payload: BatchProgress }) => void;
    let completionHandler!: (event: { payload: BatchCompletion }) => void;
    listen.mockImplementation(async (event, handler) => {
      if (event === "font-batch-progress") {
        progressHandler = handler as (event: { payload: BatchProgress }) => void;
      } else if (event === "font-batch-complete") {
        completionHandler = handler as (event: { payload: BatchCompletion }) => void;
      }
      return () => {};
    });
    const completion: BatchCompletion = {
      id: 1,
      operation: "load",
      result: {
        snapshot: {
          ...emptySnapshot(),
          fonts: [font],
          restoreCandidates: ["C:\\Fonts\\Pending.ttf"],
        },
        changed: [font.canonicalPath],
        skipped: [],
        errors: [],
      },
    };
    const finalProgress = { id: 1, operation: "load", completed: 2, total: 2 } as const;
    invoke.mockImplementation((command: string) => {
      if (command === "get_initial_state") {
        return Promise.resolve(initialState(
          { ...emptySnapshot(), fonts: [font] },
          { ...finalProgress, completed: 1 },
        ));
      }
      return Promise.resolve(undefined);
    });

    const wrapper = mount(App);
    await flushPromises();

    expect(wrapper.text()).toContain("1/2");
    expect(wrapper.findAll("[data-testid='font-row']")).toHaveLength(1);
    progressHandler({ payload: finalProgress });
    await flushPromises();
    expect(wrapper.text()).toContain("2/2");
    expect(wrapper.text()).toContain("正在完成加载");
    expect(wrapper.get(".button-primary").attributes("disabled")).toBeDefined();

    completionHandler({ payload: completion });
    progressHandler({ payload: finalProgress });
    await flushPromises();
    expect(wrapper.text()).toContain("加载成功 1 个");
    expect(wrapper.findAll("[data-testid='font-row']")).toHaveLength(1);
    expect(wrapper.get(".history-count").text()).toBe("1");
    expect(wrapper.find(".operation-status").exists()).toBe(true);
    expect(wrapper.get(".button-primary").attributes("disabled")).toBeUndefined();
    expect(invoke).toHaveBeenCalledWith("acknowledge_batch_completion", { id: 1 });
    wrapper.unmount();
  });

  it("keeps the completion when an older initial response arrives later", async () => {
    let completionHandler!: (event: { payload: BatchCompletion }) => void;
    listen.mockImplementation(async (event, handler) => {
      if (event === "font-batch-complete") {
        completionHandler = handler as (event: { payload: BatchCompletion }) => void;
      }
      return () => {};
    });
    let resolveInitial!: (state: InitialState) => void;
    const pendingInitial = new Promise<InitialState>(resolve => {
      resolveInitial = resolve;
    });
    invoke.mockImplementation((command: string) =>
      command === "get_initial_state" ? pendingInitial : Promise.resolve(undefined),
    );
    const wrapper = mount(App);
    await flushPromises();

    completionHandler({ payload: batchCompletion({
      snapshot: { ...emptySnapshot(), fonts: [font] },
      changed: [font.canonicalPath],
      skipped: [],
      errors: [],
    }) });
    resolveInitial(initialState(emptySnapshot(), {
      id: 1,
      operation: "load",
      completed: 1,
      total: 1,
    }));
    await flushPromises();

    expect(wrapper.text()).toContain("加载成功 1 个");
    expect(wrapper.findAll('[data-testid="font-row"]')).toHaveLength(1);
    expect(wrapper.get(".button-primary").attributes("disabled")).toBeUndefined();
    wrapper.unmount();
  });

  it("shows the header restore button without the old session banner", async () => {
    mockInitial({
      ...emptySnapshot(),
      restoreCandidates: ["C:\\Fonts\\Example.ttf", "C:\\Fonts\\Other.otf"],
    });
    const wrapper = mount(App);
    await flushPromises();

    expect(wrapper.find(".restore-band").exists()).toBe(false);
    expect(wrapper.text()).not.toContain("检测到上次会话");
    expect(wrapper.get('[data-testid="load-last-fonts"]').text()).toContain("加载上次字体");
    expect(wrapper.get(".history-count").text()).toBe("2");
    expect(wrapper.get(".history-count").classes()).not.toContain("is-empty");
    expect(wrapper.get('[data-testid="load-last-fonts"]').attributes("aria-label")).toBe(
      "加载上次字体，2 个字体可恢复",
    );
  });

  it("warns when loading previous fonts without history", async () => {
    vi.useFakeTimers();
    try {
      mockInitial(emptySnapshot());
      const wrapper = mount(App);
      await flushPromises();

      const button = wrapper.get('[data-testid="load-last-fonts"]');
      expect(button.attributes("disabled")).toBeUndefined();
      expect(button.get(".history-count").text()).toBe("0");
      expect(button.get(".history-count").classes()).toContain("is-empty");
      await button.trigger("click");

      expect(wrapper.get(".operation-status").text()).toContain(
        "暂无上次加载记录，请先选择字体",
      );
      expect(wrapper.get(".operation-status").classes()).toContain("is-warning");
      expect(wrapper.get(".status-summary").findComponent(AlertTriangle).exists()).toBe(true);
      expect(wrapper.get(".status-summary").findComponent(CheckCircle2).exists()).toBe(false);
      expect(invoke).not.toHaveBeenCalledWith("restore_last_session", {});
      expect(open).not.toHaveBeenCalled();

      vi.advanceTimersByTime(3000);
      await flushPromises();
      expect(wrapper.find(".operation-status").exists()).toBe(false);
      wrapper.unmount();
    } finally {
      vi.useRealTimers();
    }
  });

  it("loads all previous-session fonts from the header with progress", async () => {
    const other = {
      ...font,
      canonicalPath: "C:\\Fonts\\Other.otf",
      fileName: "Other.otf",
      format: "OTF" as const,
    };
    const paths = [font.canonicalPath, other.canonicalPath];
    let resolveBatch!: (result: BatchCompletion) => void;
    const pendingBatch = new Promise<BatchCompletion>(resolve => {
      resolveBatch = resolve;
    });
    invoke.mockImplementation((command: string) => {
      if (command === "get_initial_state") {
        return Promise.resolve(initialState({ ...emptySnapshot(), restoreCandidates: paths }));
      }
      if (command === "restore_last_session") {
        return pendingBatch;
      }
      return Promise.resolve(undefined);
    });
    const wrapper = mount(App);
    await flushPromises();

    await wrapper.get('[data-testid="load-last-fonts"]').trigger("click");
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("restore_last_session", {});
    expect(wrapper.get(".operation-status").text()).toContain("0/2");
    expect(wrapper.get('[data-testid="load-last-fonts"]').attributes("disabled")).toBeDefined();
    expect(open).not.toHaveBeenCalled();

    resolveBatch(batchCompletion({
      snapshot: { ...emptySnapshot(), fonts: [font, other] },
      changed: paths,
      skipped: [],
      errors: [],
    }));
    await flushPromises();

    expect(wrapper.findAll('[data-testid="font-row"]')).toHaveLength(2);
    expect(wrapper.get(".operation-status").text()).toContain("加载成功 2 个");
    expect(wrapper.find(".restore-band").exists()).toBe(false);
    expect(wrapper.get(".history-count").text()).toBe("0");
    expect(wrapper.get('[data-testid="load-last-fonts"]').attributes("disabled")).toBeUndefined();
    wrapper.unmount();
  });

  it("keeps failed previous-session fonts available for retry", async () => {
    const missingPath = "C:\\Fonts\\Missing.ttf";
    const restoredFont = { ...font, canonicalPath: missingPath, fileName: "Missing.ttf" };
    const restore = vi
      .fn()
      .mockResolvedValueOnce(batchCompletion({
        snapshot: { ...emptySnapshot(), fonts: [font], restoreCandidates: [missingPath] },
        changed: [font.canonicalPath],
        skipped: [],
        errors: [{ path: missingPath, code: "file_missing", message: "文件不存在" }],
      }))
      .mockResolvedValueOnce(batchCompletion({
        snapshot: { ...emptySnapshot(), fonts: [font, restoredFont] },
        changed: [missingPath],
        skipped: [],
        errors: [],
      }, "load", 2));
    invoke.mockImplementation((command: string) => {
      if (command === "get_initial_state") {
        return Promise.resolve(initialState({
          ...emptySnapshot(),
          restoreCandidates: [font.canonicalPath, missingPath],
        }));
      }
      if (command === "restore_last_session") {
        return restore();
      }
      return Promise.resolve(undefined);
    });
    const wrapper = mount(App);
    await flushPromises();

    await wrapper.get('[data-testid="load-last-fonts"]').trigger("click");
    await flushPromises();

    expect(wrapper.get(".operation-status").text()).toContain("加载成功 1 个，失败 1 个");
    expect(wrapper.get(".operation-status").classes()).toContain("is-warning");
    expect(wrapper.get(".status-summary").findComponent(AlertTriangle).exists()).toBe(true);
    expect(wrapper.get(".status-summary").findComponent(CheckCircle2).exists()).toBe(false);
    expect(wrapper.get(".error-list").text()).toContain(missingPath);
    expect(wrapper.get(".error-list").text()).toContain("文件不存在");
    expect(wrapper.findAll('[data-testid="font-row"]')).toHaveLength(1);
    expect(wrapper.get(".history-count").text()).toBe("1");

    await wrapper.get('[data-testid="load-last-fonts"]').trigger("click");
    await flushPromises();

    expect(restore).toHaveBeenCalledTimes(2);
    expect(wrapper.findAll('[data-testid="font-row"]')).toHaveLength(2);
    expect(wrapper.get(".history-count").text()).toBe("0");
    wrapper.unmount();
  });

  it("sorts fonts by name and loaded time in both directions", async () => {
    const older = {
      ...font,
      canonicalPath: "C:\\Fonts\\Zulu.ttf",
      fileName: "Zulu.ttf",
      faces: [{ family: "Zulu Sans", familyAliases: [], style: "Regular" }],
      loadedAt: "2026-08-01T12:00:00Z",
    };
    const newer = {
      ...font,
      canonicalPath: "C:\\Fonts\\Alpha.ttf",
      fileName: "Alpha.ttf",
      faces: [{ family: "Alpha Sans", familyAliases: [], style: "Regular" }],
      loadedAt: "2026-08-03T12:00:00Z",
    };
    mockInitial({ ...emptySnapshot(), fonts: [older, newer] });
    const wrapper = mount(App);
    await flushPromises();

    const names = () =>
      wrapper
        .findAll("[data-testid='font-row']")
        .map(row => row.find(".font-identity strong").text());

    expect(names()).toEqual(["Alpha Sans", "Zulu Sans"]);

    await wrapper.get('[data-testid="font-sort"]').setValue("name-desc");
    expect(names()).toEqual(["Zulu Sans", "Alpha Sans"]);

    await wrapper.get('[data-testid="font-sort"]').setValue("loaded-at-asc");
    expect(names()).toEqual(["Zulu Sans", "Alpha Sans"]);

    await wrapper.get('[data-testid="font-sort"]').setValue("loaded-at-desc");
    expect(names()).toEqual(["Alpha Sans", "Zulu Sans"]);
  });

  it("persists and restores the selected sort mode", async () => {
    mockInitial({ ...emptySnapshot(), fonts: [font] });
    const wrapper = mount(App);
    await flushPromises();

    await wrapper.get('[data-testid="font-sort"]').setValue("loaded-at-desc");
    expect(localStorage.getItem("font-loader-sort")).toBe("loaded-at-desc");
    wrapper.unmount();

    const restoredWrapper = mount(App);
    await flushPromises();

    expect(
      (restoredWrapper.get('[data-testid="font-sort"]').element as HTMLSelectElement).value,
    ).toBe("loaded-at-desc");
    restoredWrapper.unmount();
  });

  it("finds a collapsed TTC family by its alias and unloads the entire file", async () => {
    const collection = {
      ...font,
      canonicalPath: "C:\\Fonts\\Collection.ttc",
      fileName: "Collection.ttc",
      format: "TTC" as const,
      faces: ["Alpha", "Beta", "Gamma", "Hidden Family"].map(family => ({
        family,
        familyAliases: family === "Hidden Family" ? ["Localized Name"] : [],
        style: "Regular",
      })),
    };
    mockInitial({ ...emptySnapshot(), fonts: [font, collection] });
    const wrapper = mount(App);
    await flushPromises();

    expect(wrapper.findAll(".family-name").map(row => row.text())).not.toContain("Hidden Family");
    await wrapper.get('input[type="search"]').setValue("localized name");
    expect(wrapper.findAll('[data-testid="font-row"]')).toHaveLength(1);
    expect(wrapper.findAll(".family-name").map(row => row.text())).toContain("Hidden Family");
    expect(wrapper.text()).not.toContain("Localized Name");
    invoke.mockResolvedValue(batchCompletion({
      snapshot: { ...emptySnapshot(), fonts: [font] },
      changed: [collection.canonicalPath],
      skipped: [],
      errors: [],
    }, "unload"));
    await wrapper.get('button[aria-label="卸载 Collection.ttc"]').trigger("click");
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("unload_fonts", {
      paths: [collection.canonicalPath],
    });
    expect(wrapper.text()).toContain("没有匹配的字体");
    wrapper.unmount();
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
        return Promise.resolve(initialState(emptySnapshot()));
      }
      if (command === "load_fonts") {
        return Promise.resolve(batchCompletion(result));
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

  it("shows drag guidance and loads supported dropped fonts", async () => {
    let dragEnterHandler!: () => void;
    let dragDropHandler!: (event: { payload: { paths: string[] } }) => void;
    listen.mockImplementation(async (event, handler) => {
      if (event === "tauri://drag-enter") {
        dragEnterHandler = handler as () => void;
      }
      if (event === "tauri://drag-drop") {
        dragDropHandler = handler as (event: { payload: { paths: string[] } }) => void;
      }
      return () => {};
    });
    const result: BatchResult = {
      snapshot: { ...emptySnapshot(), fonts: [font] },
      changed: [font.canonicalPath],
      skipped: [],
      errors: [],
    };
    invoke.mockImplementation((command: string) => {
      if (command === "get_initial_state") {
        return Promise.resolve(initialState(emptySnapshot()));
      }
      if (command === "load_fonts") {
        return Promise.resolve(batchCompletion(result));
      }
      return Promise.resolve(undefined);
    });
    const wrapper = mount(App);
    await flushPromises();

    dragEnterHandler();
    await flushPromises();
    expect(wrapper.get(".drop-overlay").text()).toContain("拖放字体文件以导入");

    dragDropHandler({
      payload: {
        paths: ["C:\\Fonts\\A.ttf", "C:\\Fonts\\notes.txt", "C:\\Fonts\\B.OTF"],
      },
    });
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("load_fonts", {
      paths: ["C:\\Fonts\\A.ttf", "C:\\Fonts\\B.OTF"],
    });
    expect(wrapper.find(".drop-overlay").exists()).toBe(false);
    expect(wrapper.get(".drop-notice").text()).toContain("已忽略 1 个不支持的文件");
  });

  it("rejects dropped fonts during a batch without replacing its status", async () => {
    let dragEnterHandler!: () => void;
    let dragDropHandler!: (event: { payload: { paths: string[] } }) => void;
    listen.mockImplementation(async (event, handler) => {
      if (event === "tauri://drag-enter") {
        dragEnterHandler = handler as () => void;
      }
      if (event === "tauri://drag-drop") {
        dragDropHandler = handler as (event: { payload: { paths: string[] } }) => void;
      }
      return () => {};
    });
    open.mockResolvedValue(["C:\\Fonts\\A.ttf"]);
    let resolveBatch!: (result: BatchCompletion) => void;
    const pendingBatch = new Promise<BatchCompletion>(resolve => {
      resolveBatch = resolve;
    });
    invoke.mockImplementation((command: string) => {
      if (command === "get_initial_state") {
        return Promise.resolve(initialState(emptySnapshot()));
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
    expect(wrapper.get(".operation-status").text()).toContain("正在加载字体");

    dragEnterHandler();
    await flushPromises();
    expect(wrapper.get(".drop-overlay").text()).toContain("暂时无法导入字体");
    dragDropHandler({ payload: { paths: ["C:\\Fonts\\B.otf"] } });
    await flushPromises();

    expect(invoke.mock.calls.filter(([command]) => command === "load_fonts")).toHaveLength(1);
    expect(wrapper.get(".operation-status").text()).toContain("正在加载字体");
    expect(wrapper.get(".drop-notice").text()).toContain("当前正在处理字体");

    resolveBatch(batchCompletion({
      snapshot: { ...emptySnapshot(), fonts: [font] },
      changed: [font.canonicalPath],
      skipped: [],
      errors: [],
    }));
    await flushPromises();
    wrapper.unmount();
  });

  it("keeps unrelated restore candidates after choosing fonts", async () => {
    const initialSnapshot = {
      ...emptySnapshot(),
      restoreCandidates: ["C:\\Fonts\\Previous.ttf"],
    };
    open.mockResolvedValue(["C:\\Fonts\\A.ttf"]);
    invoke.mockImplementation((command: string) => {
      if (command === "get_initial_state") {
        return Promise.resolve(initialState(initialSnapshot));
      }
      if (command === "load_fonts") {
        return Promise.resolve(batchCompletion({
          snapshot: { ...initialSnapshot, fonts: [font] },
          changed: [font.canonicalPath],
          skipped: [],
          errors: [],
        }));
      }
      return Promise.resolve(undefined);
    });
    const wrapper = mount(App);
    await flushPromises();

    expect(wrapper.get(".history-count").text()).toBe("1");
    await wrapper.get(".button-primary").trigger("click");
    await flushPromises();

    expect(wrapper.get(".history-count").text()).toBe("1");
    expect(wrapper.get('[data-testid="load-last-fonts"]').attributes("disabled")).toBeUndefined();
    wrapper.unmount();
  });

  it("clears pending restore history only after confirmation", async () => {
    const initialSnapshot = {
      ...emptySnapshot(),
      restoreCandidates: ["C:\\Fonts\\Previous.ttf"],
    };
    confirm.mockResolvedValueOnce(false).mockResolvedValueOnce(true);
    invoke.mockImplementation((command: string) => {
      if (command === "get_initial_state") {
        return Promise.resolve(initialState(initialSnapshot));
      }
      if (command === "dismiss_last_session") {
        return Promise.resolve(emptySnapshot());
      }
      return Promise.resolve(undefined);
    });
    const wrapper = mount(App);
    await flushPromises();
    const clearHistory = 'button[aria-label="清除待恢复记录"]';

    await wrapper.get(clearHistory).trigger("click");
    await flushPromises();
    expect(invoke).not.toHaveBeenCalledWith("dismiss_last_session");
    expect(wrapper.get(".history-count").text()).toBe("1");

    await wrapper.get(clearHistory).trigger("click");
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith("dismiss_last_session");
    expect(wrapper.get(".history-count").text()).toBe("0");
    expect(wrapper.find(clearHistory).exists()).toBe(false);
    wrapper.unmount();
  });

  it("shows progress while loading files", async () => {
    mockInitial(emptySnapshot());
    open.mockResolvedValue(["C:\\Fonts\\A.ttf", "C:\\Fonts\\B.otf"]);
    let resolveBatch!: (result: BatchCompletion) => void;
    const pendingBatch = new Promise<BatchCompletion>(resolve => {
      resolveBatch = resolve;
    });
    let progressHandler!: (event: { payload: BatchProgress }) => void;
    listen.mockImplementation(async (event, handler) => {
      if (event === "font-batch-progress") {
        progressHandler = handler as (event: { payload: BatchProgress }) => void;
      }
      return () => {};
    });
    invoke.mockImplementation((command: string) => {
      if (command === "get_initial_state") {
        return Promise.resolve(initialState(emptySnapshot()));
      }
      if (command === "load_fonts") {
        return pendingBatch;
      }
      return Promise.resolve(undefined);
    });
    const wrapper = mount(App);
    await flushPromises();

    await wrapper.get('[data-testid="load-last-fonts"]').trigger("click");
    expect(wrapper.get(".operation-status").classes()).toContain("is-warning");
    await wrapper.get(".button-primary").trigger("click");
    await flushPromises();

    expect(wrapper.get(".operation-status").classes()).not.toContain("is-warning");
    expect(wrapper.text()).toContain("0/2");
    progressHandler({ payload: { id: 1, operation: "load", completed: 1, total: 2 } });
    await flushPromises();
    expect(wrapper.text()).toContain("1/2");
    progressHandler({ payload: { id: 1, operation: "load", completed: 2, total: 2 } });
    await flushPromises();
    expect(wrapper.text()).toContain("正在完成加载");
    expect(wrapper.get(".button-primary").attributes("disabled")).toBeDefined();

    resolveBatch(batchCompletion({
      snapshot: { ...emptySnapshot(), fonts: [font] },
      changed: [font.canonicalPath],
      skipped: [],
      errors: [],
    }));
    await flushPromises();
    expect(wrapper.text()).toContain("加载成功 1 个");
    expect(wrapper.get(".status-summary").findComponent(CheckCircle2).exists()).toBe(true);
  });

  it.each([true, false])("keeps a failed operation visible until dismissed (rejected: %s)", async rejected => {
    vi.useFakeTimers();
    try {
      mockInitial(emptySnapshot());
      open.mockResolvedValue([font.canonicalPath]);
      invoke.mockImplementation((command: string) => {
        if (command === "get_initial_state") {
          return Promise.resolve(initialState(emptySnapshot()));
        }
        if (command === "load_fonts") {
          return rejected
            ? Promise.reject(new Error("无法执行加载命令"))
            : Promise.resolve(batchCompletion({
                snapshot: emptySnapshot(),
                changed: [],
                skipped: [],
                errors: [{ path: font.canonicalPath, code: "file_missing", message: "文件不存在" }],
              }));
        }
        return Promise.resolve(undefined);
      });
      const wrapper = mount(App);
      await flushPromises();

      await wrapper.get(".button-primary").trigger("click");
      await flushPromises();

      expect(wrapper.get(".status-summary").text()).toContain("加载失败");
      expect(wrapper.get(".operation-status").classes()).toContain("is-error");
      expect(wrapper.get(".status-summary").findComponent(CircleX).exists()).toBe(true);
      vi.advanceTimersByTime(3000);
      await flushPromises();
      const details = wrapper.get(".error-list").text();
      expect(details).toContain(rejected ? "无法执行加载命令" : "文件不存在");
      if (!rejected) {
        expect(details).toContain(font.canonicalPath);
      }
      await wrapper.get(".status-dismiss").trigger("click");
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
    invoke.mockResolvedValue(batchCompletion({
      snapshot: emptySnapshot(),
      changed: [font.canonicalPath],
      skipped: [],
      errors: [],
    }, "unload"));
    await wrapper.get(".button-danger-quiet").trigger("click");
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("unload_fonts", {
      paths: [font.canonicalPath],
    });
  });

  it("shows exit retry errors alongside the original failure and supports force exit", async () => {
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

    const dialog = wrapper.get("[role='dialog']");
    expect(dialog.text()).toContain("部分字体卸载失败");
    invoke.mockRejectedValueOnce(new Error("无法请求退出"));
    const retry = dialog.get(".dialog-actions .button-primary");
    await retry.trigger("click");
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("request_exit", { force: false });
    expect(dialog.get("[role='alert']").text()).toContain("重试卸载失败。Error: 无法请求退出");
    expect(dialog.text()).toContain(font.canonicalPath);
    expect(dialog.text()).toContain("RemoveFontResourceExW 返回 0");
    expect(retry.attributes("disabled")).toBeUndefined();

    invoke.mockRejectedValueOnce("无法关闭窗口");
    const forceExit = dialog.get(".dialog-actions .button-secondary");
    await forceExit.trigger("click");
    await flushPromises();

    expect(dialog.get("[role='alert']").text()).toBe("退出失败。无法关闭窗口");

    invoke.mockResolvedValue({ exiting: true, errors: [] });
    await forceExit.trigger("click");
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("request_exit", { force: true });
    expect(wrapper.find("[role='dialog']").exists()).toBe(false);
    wrapper.unmount();
  });
});
