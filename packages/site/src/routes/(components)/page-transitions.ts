import { onNavigate } from '$app/navigation';

export function setupPageTransitions() {
  onNavigate((navigation) => {
			const from = navigation.from?.route.id;
			const to = navigation.to?.route.id;
			const betweenEditors =
				(from === '/' && to === '/playground') || (from === '/playground' && to === '/');

			if (
				!betweenEditors ||
				!document.startViewTransition ||
				window.matchMedia('(prefers-reduced-motion: reduce)').matches
			) {
				return;
			}

			return new Promise<void>((resume) => {
				try {
					const transition = document.startViewTransition(async () => {
						resume();
						await navigation.complete;
					});
					// A failed or interrupted transition must not block navigation.
					void transition.finished.catch(resume);
				} catch {
					resume();
				}
			});
		});
}
