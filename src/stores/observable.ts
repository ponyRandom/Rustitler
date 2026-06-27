export type Listener = () => void;

export interface ObservableStore<TState> {
  getState: () => TState;
  subscribe: (listener: Listener) => () => void;
}

export const createObservable = <TState>(initialState: TState) => {
  let state = initialState;
  const listeners = new Set<Listener>();

  const emit = () => {
    listeners.forEach((listener) => listener());
  };

  return {
    getState: () => state,
    setState: (nextState: TState) => {
      state = nextState;
      emit();
    },
    updateState: (updater: (state: TState) => TState) => {
      state = updater(state);
      emit();
    },
    subscribe: (listener: Listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
};
