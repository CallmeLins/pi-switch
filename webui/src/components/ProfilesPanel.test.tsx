import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ProfilesPanel } from "./ProfilesPanel";
import { LanguageProvider } from "../i18n";
import { ToastProvider } from "./ui";
import { api } from "../api";
import type { AppState } from "../types";

function stateWithProfile(overrides: Record<string, unknown> = {}) {
  return {
    current: "native",
    profiles: {
      native: {
        api: "openai-responses",
        baseUrl: "https://example.test/v1",
        apiKey: "key",
        models: [],
        proxy: false,
        responsesMode: "passthrough",
        ...overrides,
      },
    },
    settings: {},
  } as unknown as AppState;
}

function renderPanel(state = stateWithProfile()) {
  return render(
    <LanguageProvider configLang="en">
      <ToastProvider>
        <ProfilesPanel state={state} refresh={vi.fn(async () => {})} />
      </ToastProvider>
    </LanguageProvider>,
  );
}

describe("provider Responses mode form", () => {
  beforeEach(() => {
    vi.spyOn(api, "getPresets").mockResolvedValue([]);
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("shows and preserves the existing Responses mode", async () => {
    renderPanel();
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));

    await waitFor(() => expect(screen.getAllByRole("combobox")).toHaveLength(4));
    expect(screen.getAllByRole("combobox")[2]).toHaveValue("passthrough");
    expect(screen.getByText("Responses mode")).toBeInTheDocument();
  });

  it("blocks a passthrough mode on a Chat Completions provider", async () => {
    const update = vi.spyOn(api, "updateProfile").mockResolvedValue({});
    renderPanel();
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    await waitFor(() => expect(screen.getAllByRole("combobox")).toHaveLength(4));

    fireEvent.change(screen.getAllByRole("combobox")[1], {
      target: { value: "openai-completions" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(screen.getByText("passthrough requires openai-responses")).toBeInTheDocument(),
    );
    expect(update).not.toHaveBeenCalled();
  });

  it("shows the effective Responses mode on the provider list", () => {
    renderPanel(stateWithProfile({ responsesMode: "auto" }));
    expect(screen.getByText("Responses: passthrough")).toBeInTheDocument();
  });

  it("maps auto to conversion for a Chat Completions provider", () => {
    renderPanel(stateWithProfile({ api: "openai-completions", responsesMode: "auto" }));
    expect(screen.getByText("Responses: convert")).toBeInTheDocument();
  });
});
