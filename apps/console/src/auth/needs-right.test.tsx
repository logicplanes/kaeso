import { describe, expect, it } from "vitest";
import { screen } from "@testing-library/react";
import { ORG, PROJECT, membership, signedIn, whoAmI } from "~/test/fixtures";
import { memberUser } from "~/test/fixtures/public";
import { apiError, on, requests } from "~/test/network";
import { renderUi } from "~/test/render";
import { queryClient } from "~/query";
import { explainMissingRight, needsText, useNeedsRight } from "./needs-right";

const GRANTORS = "orgs/:org/rights/:right/grantors";
const PROJECT_GRANTORS = "orgs/:org/projects/:project/rights/:right/grantors";

function Reason({ right, project }: { right: string; project?: string }) {
  const needs = useNeedsRight(ORG, project);
  return <p>{needs(right)}</p>;
}

describe("saying who can give a right", () => {
  it("names nobody, one person or either of two", () => {
    expect(needsText("Export logs", undefined)).toBe("You need Export logs; ask an owner or admin for it.");
    expect(needsText("Export logs", [])).toBe("You need Export logs; ask an owner or admin for it.");
    expect(needsText("Export logs", [{ name: "Ada Okafor" }])).toBe("You need Export logs; ask Ada Okafor for it.");
    expect(needsText("Export logs", [{ name: "Ada Okafor" }, { name: "Grace Hopper" }, { name: "Linus" }])).toBe(
      "You need Export logs; ask Ada Okafor or Grace Hopper for it.",
    );
  });
});

describe("explaining a missing right", () => {
  it("names the people who can give an organization right", async () => {
    signedIn();
    on("GET", GRANTORS, { data: [memberUser("Grace Hopper")] });
    expect(await explainMissingRight(queryClient, ORG, undefined, "org.delete")).toBe("You need Delete organization; ask Grace Hopper for it.");
    expect(requests("GET", GRANTORS).map((r) => r.params.right)).toEqual(["org.delete"]);
  });

  it("names the people who can give a project right", async () => {
    signedIn();
    on("GET", PROJECT_GRANTORS, { data: [memberUser("Grace Hopper"), memberUser("Linus Torvalds")] });
    expect(await explainMissingRight(queryClient, ORG, PROJECT, "keys.all.view")).toBe("You need View all keys; ask Grace Hopper or Linus Torvalds for it.");
  });

  it("asks an owner or admin for a project right without seeing the project's members", async () => {
    signedIn({ projectRights: ["project.view", "keys.own.create"] });
    expect(await explainMissingRight(queryClient, ORG, PROJECT, "keys.all.view")).toBe("You need View all keys; ask an owner or admin for it.");
    expect(requests("GET", PROJECT_GRANTORS)).toHaveLength(0);
  });

  it("asks an owner or admin when nobody can be looked up", async () => {
    signedIn({ session: whoAmI({ memberships: [membership({ rights: ["org.view"] })] }) });
    expect(await explainMissingRight(queryClient, ORG, undefined, "org.delete")).toBe("You need Delete organization; ask an owner or admin for it.");
    expect(await explainMissingRight(queryClient, ORG, undefined, "keys.all.view")).toBe("You need View all keys; ask an owner or admin for it.");
    expect(requests("GET", GRANTORS)).toHaveLength(0);
    expect(requests("GET", PROJECT_GRANTORS)).toHaveLength(0);
  });

  it("asks an owner or admin when the people can't be read", async () => {
    signedIn();
    on("GET", GRANTORS, apiError(403, "permission_denied", "You can't see who holds rights."));
    expect(await explainMissingRight(queryClient, ORG, undefined, "org.delete")).toBe("You need Delete organization; ask an owner or admin for it.");
  });

  it("uses the right's id when the catalogue can't be read or lacks it", async () => {
    signedIn();
    on("GET", GRANTORS, { data: [] });
    expect(await explainMissingRight(queryClient, ORG, undefined, "future.right")).toBe("You need future.right; ask an owner or admin for it.");
    on("GET", "orgs/:org/rights", apiError(403, "permission_denied", "No."));
    queryClient.clear();
    expect(await explainMissingRight(queryClient, ORG, undefined, "org.delete")).toBe("You need org.delete; ask an owner or admin for it.");
  });
});

describe("the reason on a disabled action", () => {
  it("fills in the right's name and who can give it once they're known", async () => {
    signedIn();
    on("GET", PROJECT_GRANTORS, { data: [memberUser("Grace Hopper")] });
    await renderUi(<Reason right="keys.all.view" project={PROJECT} />);
    expect(await screen.findByText("You need View all keys; ask Grace Hopper for it.")).toBeInTheDocument();
    expect(requests("GET", PROJECT_GRANTORS)).toHaveLength(1);
  });

  it("skips asking for project grantors without the right to see the project's members", async () => {
    signedIn({ projectRights: ["project.view"] });
    await renderUi(<Reason right="keys.all.view" project={PROJECT} />);
    expect(await screen.findByText("You need View all keys; ask an owner or admin for it.")).toBeInTheDocument();
    expect(requests("GET", PROJECT_GRANTORS)).toHaveLength(0);
  });

  it("skips asking for project grantors outside a project", async () => {
    signedIn();
    await renderUi(<Reason right="keys.all.view" />);
    expect(await screen.findByText("You need View all keys; ask an owner or admin for it.")).toBeInTheDocument();
    expect(requests("GET", PROJECT_GRANTORS)).toHaveLength(0);
  });

  it("says the organization is read-only while it's scheduled for deletion", async () => {
    signedIn({ session: whoAmI({ memberships: [membership({ org_state: "scheduled_for_deletion" })] }) });
    await renderUi(<Reason right="org.edit" />);
    expect(await screen.findByText("The organization is scheduled for deletion")).toBeInTheDocument();
    expect(requests("GET", GRANTORS)).toHaveLength(0);
  });
});
