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
 * CreateProposal page — issue #326
 *
 * Renders the proposal creation form and handles post-submit navigation.
 * On success the user is redirected to the proposal detail page for the
 * newly created proposal.  On cancel they are sent back to the proposals list.
 */

import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { CreateProposalForm } from '../components/CreateProposalForm';

export default function CreateProposal() {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const [successId, setSuccessId] = useState<string | null>(null);

  function handleSuccess(proposalId: string) {
    setSuccessId(proposalId);
    // Navigate to the proposal detail page if we got an id; otherwise list.
    if (proposalId) {
      navigate(`/proposals/${proposalId}`);
    } else {
      navigate('/');
    }
  }

  function handleCancel() {
    navigate('/');
  }

  // Render a brief success banner while redirect fires (avoids blank flash).
  if (successId) {
    return (
      <main style={pageStyles.main} aria-live="polite">
        <p style={pageStyles.successMsg}>
          {t('createProposal.successMessage', { id: successId })}
        </p>
      </main>
    );
  }

  return (
    <main style={pageStyles.main}>
      <CreateProposalForm onSuccess={handleSuccess} onCancel={handleCancel} />
    </main>
  );
}

const pageStyles: Record<string, React.CSSProperties> = {
  main: {
    padding: '32px 16px',
    background: '#121212',
    minHeight: '100vh',
  },
  successMsg: {
    color: '#66bb6a',
    fontSize: 16,
    textAlign: 'center',
    marginTop: 48,
  },
};
