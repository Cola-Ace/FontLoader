import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import FontFamilies from "./FontFamilies.vue";
import type { FontEntry } from "./types";

const font: FontEntry = {
  canonicalPath: "C:\\Fonts\\Collection.ttc",
  fileName: "Collection.ttc",
  format: "TTC",
  faces: [
    { family: "Example Sans", style: "Regular" },
    { family: "Example Sans", style: "Bold" },
    { family: "Example Sans", style: "Bold" },
    { family: "思源黑体", style: "Regular" },
    { family: "Example Serif", style: "Light" },
    { family: "Example Mono", style: "Regular" },
  ],
  loadedAt: "2026-08-02T12:00:00Z",
};
const writeText = vi.fn();

describe("FontFamilies", () => {
  beforeEach(() => {
    writeText.mockReset().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
  });

  afterEach(() => vi.useRealTimers());

  it("groups TTC faces by exact family name and preserves unique styles", () => {
    const wrapper = mount(FontFamilies, { props: { font, searchTerm: "" } });

    expect(wrapper.get(".collection-count").text()).toBe("4 个字体族 · 6 个字体面");
    expect(wrapper.findAll(".family-name").map(row => row.text())).toEqual([
      "Example Sans", "思源黑体",
    ]);
    expect(wrapper.findAll(".family-styles")[0].text()).toBe("Regular · Bold");
    expect(wrapper.get(".family-name-line").element.children[1].className).toBe("family-styles");
    expect(wrapper.get(".family-name-line").element.children[2].classList).toContain("copy-family");
    expect(wrapper.get(".file-name").attributes("title")).toBe(font.canonicalPath);
    wrapper.unmount();
  });

  it("expands and collapses the remaining families", async () => {
    const wrapper = mount(FontFamilies, { props: { font, searchTerm: "" } });

    expect(wrapper.get(".collection-toggle").text()).toBe("展开其余 2 个字体族");
    expect(wrapper.get(".collection-toggle").attributes("aria-expanded")).toBe("false");
    await wrapper.get(".collection-toggle").trigger("click");
    expect(wrapper.findAll(".family-row")).toHaveLength(4);
    expect(wrapper.get(".collection-toggle").text()).toBe("收起");
    expect(wrapper.get(".collection-toggle").attributes("aria-expanded")).toBe("true");
    await wrapper.get(".collection-toggle").trigger("click");
    expect(wrapper.findAll(".family-row")).toHaveLength(2);
    expect(wrapper.get(".collection-toggle").text()).toBe("展开其余 2 个字体族");
    expect(wrapper.get(".collection-toggle").attributes("aria-expanded")).toBe("false");
    wrapper.unmount();
  });

  it.each([1, 2, 3])("previews at most two of %i unique families", async count => {
    const faces = font.faces.filter(face =>
      ["Example Sans", "思源黑体", "Example Serif"].slice(0, count).includes(face.family),
    );
    const wrapper = mount(FontFamilies, {
      props: { font: { ...font, faces }, searchTerm: "" },
    });

    expect(wrapper.findAll(".family-row")).toHaveLength(Math.min(count, 2));
    expect(wrapper.find(".collection-toggle").exists()).toBe(count > 2);
    if (count > 2) {
      expect(wrapper.get(".collection-toggle").text()).toBe("展开其余 1 个字体族");
      await wrapper.get(".collection-toggle").trigger("click");
      expect(wrapper.findAll(".family-row")).toHaveLength(3);
      await wrapper.get(".collection-toggle").trigger("click");
      expect(wrapper.findAll(".family-row")).toHaveLength(2);
    }
    wrapper.unmount();
  });

  it("reveals families beyond the preview when searching", async () => {
    const wrapper = mount(FontFamilies, { props: { font, searchTerm: "" } });

    await wrapper.setProps({ searchTerm: "Example Mono" });
    expect(wrapper.findAll(".family-name").map(row => row.text())).toContain("Example Mono");
    expect(wrapper.find(".collection-toggle").exists()).toBe(false);
    await wrapper.setProps({ searchTerm: "" });
    expect(wrapper.findAll(".family-row")).toHaveLength(2);
    wrapper.unmount();
  });

  it("preserves the expanded state after clearing search", async () => {
    const wrapper = mount(FontFamilies, { props: { font, searchTerm: "" } });

    await wrapper.get(".collection-toggle").trigger("click");
    await wrapper.setProps({ searchTerm: "Example Mono" });
    await wrapper.setProps({ searchTerm: "" });
    expect(wrapper.findAll(".family-row")).toHaveLength(4);
    expect(wrapper.get(".collection-toggle").attributes("aria-expanded")).toBe("true");
    wrapper.unmount();
  });

  it("copies only the exact family name and clears success feedback", async () => {
    vi.useFakeTimers();
    const wrapper = mount(FontFamilies, { props: { font, searchTerm: "" } });

    await wrapper.get('button[aria-label="复制 family name：思源黑体"]').trigger("click");
    await flushPromises();
    expect(writeText).toHaveBeenCalledWith("思源黑体");
    expect(wrapper.get('[role="status"]').text()).toBe("已复制：思源黑体");
    expect(wrapper.findAll(".is-copied")).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(2000);
    expect(wrapper.findAll(".is-copied")).toHaveLength(0);
    wrapper.unmount();
  });

  it("does not offer a copy-all action for TTC collections", async () => {
    const wrapper = mount(FontFamilies, { props: { font, searchTerm: "" } });

    expect(wrapper.find(".copy-all").exists()).toBe(false);
    await wrapper.get(".collection-toggle").trigger("click");
    expect(wrapper.find(".copy-all").exists()).toBe(false);
    expect(wrapper.findAll(".copy-family")).toHaveLength(4);
    wrapper.unmount();
  });

  it("shows a manual-copy instruction if the clipboard rejects the write", async () => {
    writeText.mockRejectedValue(new Error("Clipboard unavailable"));
    const wrapper = mount(FontFamilies, { props: { font, searchTerm: "" } });

    await wrapper.get(".copy-family").trigger("click");
    await flushPromises();
    expect(wrapper.get('[role="status"]').classes()).not.toContain("visually-hidden");
    expect(wrapper.get('[role="status"]').text()).toBe("复制失败，请选择名称后按 Ctrl+C");
    expect(wrapper.find(".is-copied").exists()).toBe(false);
    await wrapper.get(".copy-family").trigger("click");
    await flushPromises();
    expect(writeText).toHaveBeenCalledTimes(2);
    wrapper.unmount();
  });

  it("keeps a single-face font compact and copies its name without Regular", async () => {
    const wrapper = mount(FontFamilies, {
      props: {
        font: { ...font, format: "TTF", faces: [font.faces[0]] },
        searchTerm: "",
      },
    });

    expect(wrapper.findAll(".copy-family")).toHaveLength(1);
    expect(wrapper.find(".collection-count").exists()).toBe(false);
    expect(wrapper.find(".copy-all").exists()).toBe(false);
    expect(wrapper.find(".collection-toggle").exists()).toBe(false);
    expect(wrapper.get(".family-styles").text()).toBe("Regular");
    await wrapper.get(".copy-family").trigger("click");
    await flushPromises();
    expect(writeText).toHaveBeenCalledWith("Example Sans");
    wrapper.unmount();
  });
});
