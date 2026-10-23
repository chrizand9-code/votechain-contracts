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

/**
 * Tests for VoteHistory component (issue #11).
 * Covers: renders table rows, address filter, date filter,
 * state filter, CSV export trigger, empty state.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import VoteHistory from './VoteHistory';
import type { Proposal } from '../types';

// ── Fixtures ──────────────────────────────────────────────────

const proposals: Proposal[] = [
  {
    id: 'P-001',
    title: 'Alpha Proposal',
    description: 'desc',
    state: 'Active',
    createdAt: '2026-01-01',
    endAt: '2026-04-01',
    votesCount: 2,
    totalWeight: 3000,
    votes: [
      { address: 'GCFX4Q...ABC', type: 'For',     weight: 2000, votedAt: '2026-02-15' },
      { address: 'GOTHER...XYZ', type: 'Against', weight: 1000, votedAt: '2026-02-20' },
    ],
  },
  {
    id: 'P-002',
    title: 'Beta Proposal',
    description: 'desc2',
    state: 'Passed',
    createdAt: '2026-01-05',
    endAt: '2026-03-01',
    votesCount: 1,
    totalWeight: 500,
    votes: [
      { address: 'GCFX4Q...ABC', type: 'For', weight: 500, votedAt: '2026-01-20' },
    ],
  },
];

// ── Tests ─────────────────────────────────────────────────────

describe('VoteHistory', () => {
  afterEach(() => vi.restoreAllMocks());

  it('renders votes matching the default address filter', () => {
    render(<VoteHistory proposals={proposals} />);
    // Default address is 'GCFX4Q...' — both proposals have a matching vote
    expect(screen.getByText('Alpha Proposal')).toBeInTheDocument();
    expect(screen.getByText('Beta Proposal')).toBeInTheDocument();
  });

  it('shows empty state when address has no votes', async () => {
    const user = userEvent.setup();
    render(<VoteHistory proposals={proposals} />);

    const addressInput = screen.getByPlaceholderText(/enter public address/i);
    await user.clear(addressInput);
    await user.type(addressInput, 'GNOMATCH...');

    expect(screen.getByText(/no votes found/i)).toBeInTheDocument();
  });

  it('filters by date range — excludes votes outside range', async () => {
    const user = userEvent.setup();
    render(<VoteHistory proposals={proposals} />);

    const fromInput = screen.getByLabelText(/^from$/i);
    const toInput   = screen.getByLabelText(/^to$/i);

    await user.clear(fromInput);
    await user.type(fromInput, '2026-02-01');
    await user.clear(toInput);
    await user.type(toInput, '2026-02-28');

    // Only Alpha's vote (2026-02-15) falls in range; Beta's (2026-01-20) does not
    expect(screen.getByText('Alpha Proposal')).toBeInTheDocument();
    expect(screen.queryByText('Beta Proposal')).not.toBeInTheDocument();
  });

  it('filters by proposal state', async () => {
    const user = userEvent.setup();
    render(<VoteHistory proposals={proposals} />);

    const stateSelect = screen.getByLabelText(/proposal state/i);
    await user.selectOptions(stateSelect, 'Passed');

    expect(screen.getByText('Beta Proposal')).toBeInTheDocument();
    expect(screen.queryByText('Alpha Proposal')).not.toBeInTheDocument();
  });

  it('export CSV button is disabled when there are no matching votes', async () => {
    const user = userEvent.setup();
    render(<VoteHistory proposals={proposals} />);

    const addressInput = screen.getByPlaceholderText(/enter public address/i);
    await user.clear(addressInput);
    await user.type(addressInput, 'GNOMATCH...');

    const exportBtn = screen.getByRole('button', { name: /export csv/i });
    expect(exportBtn).toBeDisabled();
  });

  it('export CSV button is enabled when votes are visible', () => {
    render(<VoteHistory proposals={proposals} />);
    const exportBtn = screen.getByRole('button', { name: /export csv/i });
    expect(exportBtn).not.toBeDisabled();
  });

  it('triggers a file download when Export CSV is clicked', async () => {
    const user = userEvent.setup();

    // Stub URL.createObjectURL / revokeObjectURL
    const createObjectURL = vi.fn().mockReturnValue('blob:mock');
    const revokeObjectURL = vi.fn();
    Object.defineProperty(window, 'URL', {
      value: { createObjectURL, revokeObjectURL },
      writable: true,
      configurable: true,
    });

    // Stub anchor click by replacing the real anchor after createElement
    const originalCreateElement = document.createElement.bind(document);
    const clickSpy = vi.fn();
    vi.spyOn(document, 'createElement').mockImplementation((tag: string, ...args) => {
      const el = originalCreateElement(tag, ...args);
      if (tag === 'a') {
        vi.spyOn(el as HTMLAnchorElement, 'click').mockImplementation(clickSpy);
      }
      return el;
    });

    render(<VoteHistory proposals={proposals} />);
    const exportBtn = screen.getByRole('button', { name: /export csv/i });

    await user.click(exportBtn);

    expect(createObjectURL).toHaveBeenCalled();
    expect(clickSpy).toHaveBeenCalled();
    expect(revokeObjectURL).toHaveBeenCalled();
  });

  it('shows progress and disables export while generating the CSV', async () => {
    const user = userEvent.setup();
    Object.defineProperty(window, 'URL', {
      value: { createObjectURL: vi.fn().mockReturnValue('blob:mock'), revokeObjectURL: vi.fn() },
      writable: true,
      configurable: true,
    });
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});

    render(<VoteHistory proposals={proposals} />);
    const exportButton = screen.getByRole('button', { name: /export csv/i });
    const clickPromise = user.click(exportButton);

    expect(await screen.findByRole('progressbar', { name: 'Exporting CSV' })).toBeInTheDocument();
    expect(exportButton).toBeDisabled();
    await clickPromise;
  });

  it('shows an error alert if CSV export fails', async () => {
    const user = userEvent.setup();
    Object.defineProperty(window, 'URL', {
      value: { createObjectURL: vi.fn().mockImplementation(() => { throw new Error('export failed'); }) },
      writable: true,
      configurable: true,
    });

    render(<VoteHistory proposals={proposals} />);
    await user.click(screen.getByRole('button', { name: /export csv/i }));

    expect(await screen.findByRole('alert')).toHaveTextContent('CSV export failed');
  });
});
