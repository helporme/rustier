import type { Pane } from 'paneforge';

const CENTER = 50;
const SNAP_DISTANCE_PX = 8;

export class CenterSnap {
	pane = $state.raw<Pick<Pane, 'getSize' | 'resize'>>();
	group = $state.raw<HTMLElement | null>(null);

	onLayoutChange = () => {
		const pane = this.pane;
		if (!pane || !this.#nearCenter(pane.getSize())) return;

		// Finish PaneForge's current resize notifications before applying the snap.
		queueMicrotask(() => {
			if (this.pane === pane && this.#nearCenter(pane.getSize())) {
				pane.resize(CENTER);
			}
		});
	};

	#nearCenter(size: number) {
		const group = this.group;
		if (!group || size === CENTER) return false;

		const length = group.dataset.direction === 'vertical' ? group.clientHeight : group.clientWidth;
		return length > 0 && (Math.abs(size - CENTER) / 100) * length <= SNAP_DISTANCE_PX;
	}
}
