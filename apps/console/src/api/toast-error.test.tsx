import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderUi } from "~/test/render";
import { ApiError } from "./errors";
import { toastApiError } from "./toast-error";

const notifications = () => within(screen.getByRole("region", { name: "Notifications" }));

describe("an error toast", () => {
  it("gives the request id when the gateway failed", async () => {
    await renderUi(<p>page</p>);
    toastApiError(new ApiError({ type: "internal", status: 500, message: "Something broke on our side.", requestId: "req_42" }));
    expect((await notifications().findAllByText("Something broke on our side.")).length).toBeGreaterThan(0);
    expect((await notifications().findAllByText("Request req_42")).length).toBeGreaterThan(0);
  });

  it("leaves the request id out of a refusal", async () => {
    await renderUi(<p>page</p>);
    toastApiError(new ApiError({ type: "invalid_request", status: 400, message: "Name the key.", requestId: "req_43" }));
    expect((await notifications().findAllByText("Name the key.")).length).toBeGreaterThan(0);
    expect(notifications().queryByText("Request req_43")).not.toBeInTheDocument();
  });

  it("says the gateway can't be reached when the failure isn't an answer from it", async () => {
    await renderUi(<p>page</p>);
    toastApiError(new SyntaxError("Unexpected end of JSON input"));
    expect((await notifications().findAllByText("Can't reach the gateway. Try again.")).length).toBeGreaterThan(0);
    expect(notifications().queryByText("Unexpected end of JSON input")).not.toBeInTheDocument();
  });

  it("uses the caller's words for a failure that isn't an answer from the gateway", async () => {
    await renderUi(<p>page</p>);
    toastApiError(new TypeError("network"), { fallback: "The export stopped before the end. Try again." });
    expect((await notifications().findAllByText("The export stopped before the end. Try again.")).length).toBeGreaterThan(0);
  });

  it("shows the API's message for a missing right outside an organization", async () => {
    await renderUi(<p>page</p>, { path: "/settings/account/profile" });
    toastApiError(new ApiError({ type: "permission_denied", status: 403, message: "Only instance admins can do this.", right: "instance.manage" }));
    expect((await notifications().findAllByText("Only instance admins can do this.")).length).toBeGreaterThan(0);
  });
});
