import type { Pane } from 'paneforge';

const DEFAULT_HEIGHT = 232;
const KEYBOARD_STEP = 48;
const GRIP_HEIGHT = 14;
const DIVIDER_HEIGHT = 1;
const REVEAL_DISTANCE = 48;
export const MAX_TERMINAL_SIZE = 80;

export class Terminal {
	#dragGrip = $state(1);

	pane = $state.raw<Pick<Pane, 'getSize' | 'resize' | 'collapse'>>();
	element = $state.raw<HTMLElement | null>(null);
	handleHeight = $state(0);
	groupHeight = $state(0);
	visible = $state(false);
	dragging = $state(false);
	gripVisibility = $derived(this.dragging ? this.#dragGrip : this.visible ? 0 : 1);
	handleSize = $derived(DIVIDER_HEIGHT + (GRIP_HEIGHT - DIVIDER_HEIGHT) * this.gripVisibility);
	availableHeight = $derived(Math.max(0, this.groupHeight - this.handleHeight));
	// Drag continuously; apply the collapse threshold only when the pointer is released.
	readonly minSize = 0;

	#drag?: {
		pointer: number;
		y: number;
		height: number;
		handleHeight: number;
		fadeOrigin: number;
		anchor: number;
		currentHeight: number;
	};
	#suppressClick = false;

	get compactHandle() {
		return this.gripVisibility === 0;
	}

	show = () => {
		if (!this.visible) this.#resize(DEFAULT_HEIGHT);
	};

	hide = () => this.pane?.collapse();

	onResize = (size: number) => {
		this.visible = size > 0;
	};

	onPointerDown = (event: PointerEvent & { currentTarget: HTMLElement }) => {
		if (!event.isPrimary || event.button !== 0 || !this.pane) return;

		const height = this.element?.getBoundingClientRect().height ?? 0;
		const handle = event.currentTarget.getBoundingClientRect();
		this.#dragGrip = Math.max(
			0,
			Math.min(1, (handle.height - DIVIDER_HEIGHT) / (GRIP_HEIGHT - DIVIDER_HEIGHT))
		);
		this.dragging = true;
		this.#suppressClick = false;
		this.#drag = {
			pointer: event.pointerId,
			y: event.clientY,
			height,
			handleHeight: handle.height,
			fadeOrigin:
				this.#dragGrip > 0
					? REVEAL_DISTANCE * (1 - this.#dragGrip)
					: Math.max(REVEAL_DISTANCE, height),
			anchor: Math.max(0, Math.min(1, (event.clientY - handle.top) / handle.height)),
			currentHeight: height
		};
		// Freeze an in-flight opening transition at the height the user grabbed.
		this.#resize(height, handle.height);
		event.currentTarget.setPointerCapture(event.pointerId);
	};

	onPointerMove = (event: PointerEvent) => {
		const drag = this.#drag;
		if (!drag || drag.pointer !== event.pointerId) return;

		const delta = drag.y - event.clientY;
		if (Math.abs(delta) >= 4) this.#suppressClick = true;

		const height = drag.height + delta;
		this.#dragGrip = Math.max(0, Math.min(1, 1 - (drag.fadeOrigin + delta) / REVEAL_DISTANCE));
		// Keep the grabbed point under the cursor as the 14 px grip becomes a 1 px divider.
		drag.currentHeight = Math.max(
			0,
			height + (1 - drag.anchor) * (drag.handleHeight - this.handleSize)
		);
		this.#resize(drag.currentHeight, this.handleSize);
	};

	onPointerEnd = (event: PointerEvent) => {
		const drag = this.#drag;
		if (!drag || event.pointerId !== drag.pointer) return;

		if (drag.currentHeight < REVEAL_DISTANCE) this.hide();
		this.#drag = undefined;
		this.dragging = false;
	};

	onClick = (event: MouseEvent) => {
		if (event.detail !== 0 && this.#suppressClick) {
			this.#suppressClick = false;
			return;
		}

		if (this.visible) this.hide();
		else this.show();
	};

	onKeyDown = (event: KeyboardEvent) => {
		if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return;

		event.preventDefault();
		const height = ((this.pane?.getSize() ?? 0) / 100) * this.availableHeight;
		const nextHeight = height + (event.key === 'ArrowUp' ? KEYBOARD_STEP : -KEYBOARD_STEP);
		if (nextHeight < REVEAL_DISTANCE) this.hide();
		else this.#resize(nextHeight);
	};

	#resize(height: number, handleHeight = height > 0 ? DIVIDER_HEIGHT : GRIP_HEIGHT) {
		const available = this.groupHeight - handleHeight;
		if (available > 0) this.pane?.resize(Math.max(0, (height / available) * 100));
	}
}
