import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import MobPanel from './MobPanel.svelte';
import type { MobView } from '../services/alarms';

const T0 = Date.parse('2026-11-14T10:00:00Z');

const mob: MobView = {
  position: { latitudeDeg: -17.5, longitudeDeg: -149.6 },
  markedAt: T0,
  approximate: false,
  distanceM: 185.2,
  bearingRad: Math.PI,
};

function setup(
  over: Partial<MobView> = {},
  stale = false,
  state: 'active' | 'acknowledged' = 'active',
) {
  const props = {
    mob: { ...mob, ...over },
    alarmState: state,
    now: T0 + 187_000,
    stale,
    onacknowledge: vi.fn(),
    onend: vi.fn(() => Promise.resolve()),
  };
  render(MobPanel, props);
  return props;
}

describe('panneau Homme à la mer', () => {
  it('affiche en grand relèvement, distance et temps écoulé', async () => {
    const props = setup();
    expect(screen.getByTestId('mob-bearing').textContent).toBe('180°');
    expect(screen.getByTestId('mob-distance').textContent).toMatch(/0\.10/);
    expect(screen.getByText(/depuis 3 min 07 s/)).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: "Couper l'alarme" }));
    expect(props.onacknowledge).toHaveBeenCalled();
  });

  it('ne montre pas de valeur sur une position obsolète', () => {
    setup({}, true, 'acknowledged');
    expect(screen.getByTestId('mob-bearing').textContent).toBe('—');
    expect(screen.getByTestId('mob-distance').textContent).toBe('—');
    expect(screen.getByTestId('mob-stale')).toBeTruthy();
    expect(screen.queryByRole('button', { name: "Couper l'alarme" })).toBeNull();
  });

  it('signale un point approximatif et une distance inconnue', () => {
    setup({ approximate: true, distanceM: null, bearingRad: null });
    expect(screen.getByText(/Point MOB approximatif/)).toBeTruthy();
    expect(screen.getByTestId('mob-stale')).toBeTruthy();
  });

  it('demande une confirmation avant de terminer', async () => {
    const props = setup();
    const end = screen.getByRole('button', { name: 'Fin du MOB' });
    await fireEvent.click(end);
    expect(props.onend).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole('button', { name: 'Confirmer la fin du MOB' }));
    expect(props.onend).toHaveBeenCalledOnce();
  });
});
