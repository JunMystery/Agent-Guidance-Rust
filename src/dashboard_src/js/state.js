export const DEFAULT_POLL_INTERVAL = 5000;
export let pollBackoff = DEFAULT_POLL_INTERVAL;

export function setBackoff(v) { pollBackoff = v; }
export function resetBackoff() { pollBackoff = DEFAULT_POLL_INTERVAL; }

// Latest datasets, retained client-side so sort/filter re-renders without refetch.
export const store = {
  recent_actions: [],
  tool_breakdown: [],
  last_stats_hash: null,
  is_fetching: false,
};
