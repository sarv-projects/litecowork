export type TaskAttentionLoadState = {
  generation: number;
  loadingMore: boolean;
};

export type TaskAttentionLoadAction =
  | { type: "invalidate"; generation: number }
  | { type: "start-more"; generation: number }
  | { type: "finish-more"; generation: number };

/** Keeps the pagination busy state scoped to the request generation that owns it. */
export function taskAttentionLoadReducer(
  state: TaskAttentionLoadState,
  action: TaskAttentionLoadAction,
): TaskAttentionLoadState {
  switch (action.type) {
    case "invalidate":
      return { generation: action.generation, loadingMore: false };
    case "start-more":
      return state.generation === action.generation ? { ...state, loadingMore: true } : state;
    case "finish-more":
      return state.generation === action.generation ? { ...state, loadingMore: false } : state;
  }
}
