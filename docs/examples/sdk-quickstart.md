# SDK Quick-Start Guide

## Install

```bash
npm install @votechain/sdk
```

## Connect

```ts
import { VoteChainClient } from '@votechain/sdk';
const client = new VoteChainClient({ rpcUrl: process.env.RPC_URL! });
await client.connect();
```

## Read proposals

```ts
const proposals = await client.listProposals();
console.log(proposals);
```

## Submit a vote

```ts
try {
  const tx = await client.castVote({ proposalId: 1, choice: 'yes' });
  console.log('tx', tx.hash);
} catch (err) {
  console.error('vote failed', err);
}
```

## Listen for events

```ts
client.on('voteCast', (e) => console.log('vote', e));
```

## Error handling

Wrap calls in try/catch and inspect `err.code`. Retry on network errors with backoff.
