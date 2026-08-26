import { describe, it, expect } from "vitest";
import { createConfirmController } from "./pure.js";

describe("createConfirmController", () => {
  it("clickSend resolves promise with true", async () => {
    const ctl = createConfirmController();
    const promise = ctl.showConfirmModal();
    ctl.clickSend();
    expect(await promise).toBe(true);
  });

  it("clickCancel resolves promise with false", async () => {
    const ctl = createConfirmController();
    const promise = ctl.showConfirmModal();
    ctl.clickCancel();
    expect(await promise).toBe(false);
  });

  it("closeConfirmModal resolves with false (Escape or cleanup)", async () => {
    const ctl = createConfirmController();
    const promise = ctl.showConfirmModal();
    ctl.closeConfirmModal();
    expect(await promise).toBe(false);
  });

  it("isPending is true after showConfirmModal, false after clickSend", () => {
    const ctl = createConfirmController();
    expect(ctl.isPending()).toBe(false);
    ctl.showConfirmModal();
    expect(ctl.isPending()).toBe(true);
    ctl.clickSend();
    expect(ctl.isPending()).toBe(false);
  });

  it("isPending is false after clickCancel", () => {
    const ctl = createConfirmController();
    ctl.showConfirmModal();
    ctl.clickCancel();
    expect(ctl.isPending()).toBe(false);
  });

  it("clickSend after closeConfirmModal still resolves with true (promise not replaced)", async () => {
    const ctl = createConfirmController();
    const promise = ctl.showConfirmModal();
    ctl.closeConfirmModal();
    ctl.clickSend();
    expect(await promise).toBe(false);
  });

  it("second showConfirmModal cancels first with false", async () => {
    const ctl = createConfirmController();
    const first = ctl.showConfirmModal();
    const second = ctl.showConfirmModal();
    ctl.clickSend();
    expect(await first).toBe(false);
    expect(await second).toBe(true);
  });
});
