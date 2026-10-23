// Copyright 2024 VoteChain Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import { useEffect, useRef } from "react";

export type VoteChoice = "Yes" | "No" | "Abstain";

type Props = {
  proposalTitle: string;
  choice: VoteChoice;
  estimatedFee: string;
  onConfirm: () => void;
  onCancel: () => void;
};

export function VoteConfirmationDialog({
  proposalTitle,
  choice,
  estimatedFee,
  onConfirm,
  onCancel,
}: Props) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const onCancelRef = useRef(onCancel);

  useEffect(() => {
    onCancelRef.current = onCancel;
  }, [onCancel]);

  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    cancelRef.current?.focus();

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        onCancelRef.current();
        return;
      }
      if (event.key !== "Tab" || !dialogRef.current) return;

      const focusable = Array.from(
        dialogRef.current.querySelectorAll<HTMLElement>(
          "a[href], button:not([disabled]), input:not([disabled]):not([type='hidden']), select:not([disabled]), textarea:not([disabled]), [contenteditable='true'], [tabindex]:not([tabindex='-1'])",
        ),
      ).filter((element) => element.tabIndex >= 0 && element.getAttribute("aria-hidden") !== "true");
      if (focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (!dialogRef.current.contains(document.activeElement)) {
        event.preventDefault();
        (event.shiftKey ? last : first).focus();
        return;
      }
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      if (opener?.isConnected) opener.focus();
    };
  }, []);

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onCancel}>
      <div
        ref={dialogRef}
        className="vote-confirmation-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="vote-confirmation-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <h2 id="vote-confirmation-title">Confirm your vote</h2>
        <dl>
          <div>
            <dt>Proposal</dt>
            <dd>{proposalTitle}</dd>
          </div>
          <div>
            <dt>Choice</dt>
            <dd>{choice}</dd>
          </div>
          <div>
            <dt>Estimated transaction fee</dt>
            <dd>{estimatedFee}</dd>
          </div>
        </dl>
        <p>This vote will be submitted on-chain and cannot be undone.</p>
        <div className="modal-actions">
          <button ref={cancelRef} type="button" onClick={onCancel}>
            Cancel
          </button>
          <button type="button" onClick={onConfirm}>
            Confirm Vote
          </button>
        </div>
      </div>
    </div>
  );
}