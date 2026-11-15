import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import MapContextMenu from './MapContextMenu.svelte';

const position = { latitudeDeg: -17.535, longitudeDeg: -149.569 };

function setup(canPlaceBoat: boolean) {
  const props = {
    position,
    x: 100,
    y: 80,
    canPlaceBoat,
    onplaceboat: vi.fn(),
    onmeasure: vi.fn(),
    onclose: vi.fn(),
  };
  render(MapContextMenu, props);
  return props;
}

describe('menu du clic droit', () => {
  it('affiche la position et place le bateau en simulateur', async () => {
    const props = setup(true);
    expect(screen.getByTestId('context-position').textContent).toContain('17°32.100′ S');
    await fireEvent.click(screen.getByRole('menuitem', { name: 'Bateau ici' }));
    expect(props.onclose).toHaveBeenCalled();
    expect(props.onplaceboat).toHaveBeenCalledWith(position);
  });

  it('refuse « bateau ici » hors simulateur mais permet la mesure', async () => {
    const props = setup(false);
    const boat = screen.getByRole('menuitem', { name: 'Bateau ici' }) as HTMLButtonElement;
    expect(boat.disabled).toBe(true);
    expect(boat.title).toMatch(/Simulateur/);
    await fireEvent.click(screen.getByRole('menuitem', { name: 'Mesurer depuis ici' }));
    expect(props.onmeasure).toHaveBeenCalledWith(position);
    expect(props.onplaceboat).not.toHaveBeenCalled();
  });

  it('se ferme par Échap ou un clic ailleurs', async () => {
    const props = setup(true);
    await fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
    expect(props.onclose).toHaveBeenCalledTimes(1);
    await fireEvent.pointerDown(document.body);
    expect(props.onclose).toHaveBeenCalledTimes(2);
  });
});

describe('marque depuis le menu', () => {
  it('pose une marque au point visé si proposé', async () => {
    const onmark = vi.fn();
    const onclose = vi.fn();
    render(MapContextMenu, {
      position,
      x: 0,
      y: 0,
      canPlaceBoat: false,
      onplaceboat: vi.fn(),
      onmeasure: vi.fn(),
      onmark,
      onclose,
    });
    await fireEvent.click(screen.getByRole('menuitem', { name: 'Marque ici' }));
    expect(onclose).toHaveBeenCalled();
    expect(onmark).toHaveBeenCalledWith(position);
  });

  it('n’affiche pas « Marque ici » sans gestionnaire', () => {
    setup(true);
    expect(screen.queryByRole('menuitem', { name: 'Marque ici' })).toBeNull();
  });
});

describe('distance, relèvement et navigation depuis le menu', () => {
  function menu(extra: Record<string, unknown>) {
    const props = {
      position,
      x: 0,
      y: 0,
      canPlaceBoat: false,
      onplaceboat: vi.fn(),
      onmeasure: vi.fn(),
      onclose: vi.fn(),
      ongoto: vi.fn(),
      ...extra,
    };
    render(MapContextMenu, props);
    return props;
  }

  it('affiche distance et relèvement puis navigue jusqu’au point', async () => {
    const props = menu({ fromBoat: { kind: 'ok', distanceM: 2 * 1852, bearingRad: Math.PI / 2 } });
    const text = screen.getByTestId('context-from-boat').textContent ?? '';
    expect(text).toContain('2.0 NM');
    expect(text).toContain('090°');
    await fireEvent.click(screen.getByRole('menuitem', { name: "Naviguer jusqu'ici" }));
    expect(props.onclose).toHaveBeenCalled();
    expect(props.ongoto).toHaveBeenCalledWith(position);
  });

  it('signale l’attente et l’indisponibilité, bloque la navigation', () => {
    menu({ fromBoat: { kind: 'pending' } });
    expect(screen.getByTestId('context-from-boat').textContent).toMatch(/relèvement…/);
    document.body.innerHTML = '';
    menu({
      fromBoat: { kind: 'unavailable', reason: 'Position du bateau inconnue ou obsolète' },
      goToBlocked: 'Position du bateau inconnue ou obsolète',
    });
    expect(screen.getByTestId('context-from-boat').textContent).toMatch(/obsolète/);
    const go = screen.getByRole('menuitem', { name: "Naviguer jusqu'ici" }) as HTMLButtonElement;
    expect(go.disabled).toBe(true);
    expect(go.title).toMatch(/obsolète/);
  });
});
