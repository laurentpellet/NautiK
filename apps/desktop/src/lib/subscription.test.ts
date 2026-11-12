import { describe, expect, it, vi } from 'vitest';
import { holdSubscription } from './subscription';

describe('holdSubscription', () => {
  it('se désabonne au nettoyage', async () => {
    const unlisten = vi.fn();
    const cleanup = holdSubscription(() => Promise.resolve(unlisten), vi.fn());
    await Promise.resolve();
    expect(unlisten).not.toHaveBeenCalled();
    cleanup();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('se désabonne si le nettoyage précède l’abonnement', async () => {
    const unlisten = vi.fn();
    let resolve: (u: () => void) => void = () => {};
    const cleanup = holdSubscription(() => new Promise<() => void>((r) => (resolve = r)), vi.fn());
    cleanup();
    resolve(unlisten);
    await vi.waitFor(() => expect(unlisten).toHaveBeenCalledTimes(1));
  });

  it("signale l'échec de l'abonnement", async () => {
    const onError = vi.fn();
    holdSubscription(() => Promise.reject(new Error('ipc')), onError);
    await vi.waitFor(() => expect(onError).toHaveBeenCalledWith(new Error('ipc')));
  });
});
