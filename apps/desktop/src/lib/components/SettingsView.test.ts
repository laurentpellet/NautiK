import { createRawSnippet } from 'svelte';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import SettingsView, { loadSection } from './SettingsView.svelte';
import { SimulatorApi } from '../services/simulator';

const block = (text: string) => createRawSnippet(() => ({ render: () => `<p>${text}</p>` }));

function setup(withOptional = true) {
  const simulator = new SimulatorApi(() => Promise.resolve({ speedMps: 4.1156 }));
  render(SettingsView, {
    simulator,
    status: block('alertes'),
    sources: block('réglages source'),
    ...(withOptional
      ? { alarms: block('réglages alarmes'), charts: block('dossiers cartes') }
      : {}),
  });
}

const panelOf = (text: string) => screen.getByText(text).closest('[role="tabpanel"]');

describe('onglet Paramètres', () => {
  beforeEach(() => localStorage.removeItem('nautik.settingsSection'));

  it('affiche une section à la fois, Général par défaut', () => {
    setup();
    expect(screen.getAllByRole('tab').map((t) => t.querySelector('.label')?.textContent)).toEqual([
      'Général',
      'Instruments',
      'Source',
      'Alarmes',
      'Cartes',
    ]);
    expect(screen.getByRole('tab', { name: /^Général/ }).getAttribute('aria-selected')).toBe(
      'true',
    );
    expect(screen.getByText('Unités').closest('[role="tabpanel"]')?.hasAttribute('hidden')).toBe(
      false,
    );
    expect(panelOf('réglages source')?.hasAttribute('hidden')).toBe(true);
    // Les alertes de sûreté restent visibles quelle que soit la section.
    expect(screen.getByText('alertes').closest('[role="tabpanel"]')).toBeNull();
  });

  it('change de section au clic et au clavier, et la mémorise', async () => {
    setup();
    await fireEvent.click(screen.getByRole('tab', { name: /^Cartes/ }));
    expect(panelOf('dossiers cartes')?.hasAttribute('hidden')).toBe(false);
    expect(localStorage.getItem('nautik.settingsSection')).toBe('charts');

    await fireEvent.keyDown(screen.getByRole('tablist'), { key: 'ArrowDown' });
    expect(screen.getByRole('tab', { name: /^Général/ }).getAttribute('aria-selected')).toBe(
      'true',
    );
    await fireEvent.keyDown(screen.getByRole('tablist'), { key: 'ArrowUp' });
    expect(localStorage.getItem('nautik.settingsSection')).toBe('charts');
  });

  it('omet les sections facultatives absentes', () => {
    setup(false);
    expect(screen.getAllByRole('tab')).toHaveLength(3);
  });

  it('relit la section mémorisée ; valeur inconnue : Général', () => {
    expect(loadSection({ getItem: () => 'alarms' })).toBe('alarms');
    expect(loadSection({ getItem: () => 'inconnue' })).toBe('general');
    expect(loadSection({ getItem: () => null })).toBe('general');
    const throwing = {
      getItem: () => {
        throw new Error('indisponible');
      },
    };
    expect(loadSection(throwing)).toBe('general');
  });
});
