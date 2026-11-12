import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import App from './App.svelte';

const valid = {
  name: 'NautiK',
  version: '0.1.0',
  protocolVersion: 1,
  safetyNotice: "Ce logiciel n'est pas un ECDIS certifié.",
};

describe('App', () => {
  it("affiche la version du moteur et l'avertissement de sûreté", async () => {
    render(App, { invoke: () => Promise.resolve(valid) });
    const status = await screen.findByTestId('engine-status');
    expect(status.textContent).toContain('Moteur 0.1.0');
    expect(screen.getByRole('note').textContent).toContain('ECDIS');
  });

  it("signale clairement l'indisponibilité du moteur", async () => {
    render(App, { invoke: () => Promise.reject(new Error('ipc down')) });
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('Moteur indisponible : ipc down');
  });

  it('signale une réponse moteur invalide', async () => {
    render(App, { invoke: () => Promise.resolve({ name: 'NautiK' }) });
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('invalide');
  });
});
