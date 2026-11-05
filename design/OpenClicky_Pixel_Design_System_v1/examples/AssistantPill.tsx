import type { ReactNode } from 'react';

/** Presentational component only: native mic, capture and agents are wired separately. */
export type AssistantState = 'idle' | 'listening' | 'processing' | 'muted';
export type AssistantPillProps = {
  state: AssistantState;
  onPrimaryAction: () => void;
  onOpen: () => void;
  assetBase?: string;
  shortcutLabel?: string;
  children?: ReactNode;
};

const statuses: Record<AssistantState, string> = {
  idle: 'Microphone off',
  listening: 'Microphone listening',
  processing: 'Processing request',
  muted: 'Microphone muted',
};

export function AssistantPill({
  state, onPrimaryAction, onOpen, assetBase = '/openclicky-assets',
  shortcutLabel = 'Configure shortcut', children,
}: AssistantPillProps) {
  return (
    <div className="oc-pill" role="group" aria-label="OpenClicky assistant">
      <button className="oc-pill__mark" type="button" onClick={onOpen} aria-label="Open OpenClicky">
        :)
      </button>
      <button
        className="oc-pill__microphone"
        type="button"
        onClick={onPrimaryAction}
        aria-label={state === 'listening' ? 'Stop listening' : 'Microphone action'}
        disabled={state === 'processing'}
      >
        <img width="24" height="24" style={{ imageRendering: 'pixelated' }}
          src={`${assetBase}/microphone-states/png/${state}-64.png`} alt="" />
      </button>
      <span className="oc-pill__status" role="status" aria-live="polite">{statuses[state]}</span>
      {children}
      <span className="oc-pill__shortcut" aria-label={`Shortcut: ${shortcutLabel}`}>
        {shortcutLabel}
      </span>
    </div>
  );
}
