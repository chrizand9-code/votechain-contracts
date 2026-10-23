import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { VoteConfirmationDialog } from "./VoteConfirmationDialog";

function DialogHarness() {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <>
      <button type="button" onClick={() => setIsOpen(true)}>
        Open vote confirmation
      </button>
      {isOpen && (
        <VoteConfirmationDialog
          proposalTitle="Treasury proposal"
          choice="Yes"
          estimatedFee="0.00001 XLM"
          onConfirm={() => setIsOpen(false)}
          onCancel={() => setIsOpen(false)}
        />
      )}
    </>
  );
}

describe("VoteConfirmationDialog", () => {
  it("traps tab focus, closes on Escape, and restores focus to its opener", async () => {
    const user = userEvent.setup();
    render(<DialogHarness />);

    const opener = screen.getByRole("button", { name: "Open vote confirmation" });
    await user.click(opener);

    const dialog = screen.getByRole("dialog", { name: "Confirm your vote" });
    const cancelButton = screen.getByRole("button", { name: "Cancel" });
    const confirmButton = screen.getByRole("button", { name: "Confirm Vote" });
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(dialog).toHaveAttribute("aria-labelledby", "vote-confirmation-title");
    expect(cancelButton).toHaveFocus();

    await user.tab({ shift: true });
    expect(confirmButton).toHaveFocus();
    await user.keyboard("{Escape}");

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(opener).toHaveFocus();
  });
});