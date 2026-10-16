import { describe, expect, it } from "vitest";
import { screen } from "@testing-library/react";
import { ORG, PROJECT, membership, signedIn, whoAmI } from "~/test/fixtures";
import { renderUi } from "~/test/render";
import { useOrgGiver, useProjectGiver } from "./givers";

function OrgGiver() {
  const giver = useOrgGiver(ORG);
  return <p>{`${[...giver.held].join(",")} | ${[...(giver.everyProject ?? [])].join(",")} | ${giver.isOwner ? "owner" : "not owner"}`}</p>;
}

function ProjectGiver() {
  const giver = useProjectGiver(ORG, PROJECT);
  return <p>{`${[...giver.held].join(",") || "none"} | ${giver.isOwner ? "owner" : "not owner"}`}</p>;
}

describe("what the person can give", () => {
  it("holds the organization rights of their membership", async () => {
    signedIn({ session: whoAmI({ memberships: [membership({ rights: ["org.view", "members.view"], project_rights: ["keys.all.view"] })] }) });
    await renderUi(<OrgGiver />);
    expect(await screen.findByText("org.view,members.view | keys.all.view | owner")).toBeInTheDocument();
  });

  it("holds the project rights the gateway lists", async () => {
    signedIn({
      session: whoAmI({ memberships: [membership({ role: { id: "admin", kind: "builtin", name: "Admin" } })] }),
      projectRights: ["usage.view", "keys.all.view"],
    });
    await renderUi(<ProjectGiver />);
    expect(await screen.findByText("usage.view,keys.all.view | not owner")).toBeInTheDocument();
  });
});
