const ENTER_DURATION = 320;
const DWELL_DURATION = 80;
const LEAVE_DURATION = 420;
const FRAME_INTERVAL = 1000 / 30;
const MAX_FRAME_STEP = 34;
const MAX_CELLS = 6000;
const RUN_LIMIT = 16;
const CHARACTERS = ' .:-=+*#%@';
const SINE_SIZE = 4096;
const SINE_MASK = SINE_SIZE - 1;
const SINE_SCALE = SINE_SIZE / (Math.PI * 2);
const SINE = Float32Array.from({ length: SINE_SIZE }, (_, index) => Math.sin(index / SINE_SCALE));

export class PlasmaRenderer {
	#canvas: HTMLCanvasElement;
	#context: CanvasRenderingContext2D;
	#swatches: HTMLElement;
	#atlas: HTMLCanvasElement;
	#glyphs: CanvasRenderingContext2D;
	#onRunning: (running: boolean) => void;
	#frame = 0;
	#previous = 0;
	#time = 0;
	#enteredAt = 0;
	#enterFrom = 0;
	#exitAt: number | null = null;
	#returning = false;
	#running = false;
	#pending = false;
	#reduced = false;
	#visible = false;
	#disposed = false;
	#width = 0;
	#height = 0;
	#ratio = 1;
	#deviceRatio = 0;
	#cellWidth = 0;
	#cellHeight = 0;
	#pixelWidth = 0;
	#pixelHeight = 0;
	#columns = 0;
	#rows = 0;
	#paletteDirty = true;
	#layoutDirty = true;
	#ranksDirty = true;
	#baseSize = 14;
	#family = 'monospace';
	#baseWidth = 8;
	#palette: { background: string; foreground: string }[] = [];
	#atlasKey = '';
	#seed = 0;
	#origin = { x: 0.5, y: 0.5 };
	#destination = { x: 0.5, y: 0.5 };
	#horizontal = new Float32Array(0);
	#vertical = new Float32Array(0);
	#radial = new Float32Array(0);
	#entryRanks = new Float32Array(0);
	#exitRanks = new Float32Array(0);

	static create(
		canvas: HTMLCanvasElement,
		swatches: HTMLElement,
		onRunning: (running: boolean) => void
	) {
		const context = canvas.getContext('2d');
		const atlas = document.createElement('canvas');
		const glyphs = atlas.getContext('2d');

		if (!context || !glyphs) return;

		return new PlasmaRenderer(canvas, context, swatches, atlas, glyphs, onRunning);
	}

	private constructor(
		canvas: HTMLCanvasElement,
		context: CanvasRenderingContext2D,
		swatches: HTMLElement,
		atlas: HTMLCanvasElement,
		glyphs: CanvasRenderingContext2D,
		onRunning: (running: boolean) => void
	) {
		this.#canvas = canvas;
		this.#context = context;
		this.#swatches = swatches;
		this.#atlas = atlas;
		this.#glyphs = glyphs;
		this.#onRunning = onRunning;
	}

	update(pending: boolean, reduced: boolean) {
		this.#pending = pending;
		this.#reduced = reduced;

		if (reduced) {
			this.#stop();
			return;
		}

		if (pending) {
			if (!this.#running) {
				this.#time = 0;
				this.#enteredAt = 0;
				this.#enterFrom = 0;
				this.#returning = false;
				this.#origin = this.#randomPoint(this.#origin);
				this.#destination = this.#randomPoint(this.#origin);
				this.#seed = Math.random() * 100;
				this.#ranksDirty = true;
				this.#running = true;
				this.#onRunning(true);
			} else if (this.#exitAt !== null) {
				this.#enterFrom = this.#coverage();
				this.#returning ||= this.#time >= this.#exitAt;
				this.#enteredAt = this.#time;
			}

			this.#exitAt = null;
			this.#schedule();
			return;
		}

		if (!this.#running) return;

		if (!this.#visible) {
			this.#stop();
			return;
		}

		this.#exitAt = Math.max(this.#time, this.#enteredAt + ENTER_DURATION + DWELL_DURATION);
		this.#schedule();
	}

	resize(width: number, height: number) {
		if (this.#width === width && this.#height === height) return;

		this.#width = width;
		this.#height = height;
		this.#layoutDirty = true;
		this.#schedule();
	}

	setVisible(visible: boolean) {
		this.#visible = visible;

		if (!visible) {
			if (this.#pending) this.#pause();
			else this.#stop();
		} else {
			this.#schedule();
		}
	}

	invalidatePalette() {
		this.#paletteDirty = true;
		this.#atlasKey = '';
		this.#schedule();
	}

	destroy() {
		this.#disposed = true;
		this.#pause();
		this.#canvas.width = this.#canvas.height = 0;
		this.#atlas.width = this.#atlas.height = 0;
	}

	#randomPoint(previous: { x: number; y: number }) {
		let point = { x: 0.1 + Math.random() * 0.8, y: 0.1 + Math.random() * 0.8 };

		for (let attempt = 0; attempt < 8; attempt++) {
			if (Math.hypot(point.x - previous.x, point.y - previous.y) >= 0.25) return point;

			point = { x: 0.1 + Math.random() * 0.8, y: 0.1 + Math.random() * 0.8 };
		}

		return { x: previous.x < 0.5 ? 0.8 : 0.2, y: previous.y < 0.5 ? 0.8 : 0.2 };
	}

	#coverage() {
		if (this.#exitAt !== null && this.#time >= this.#exitAt) {
			const progress = Math.min((this.#time - this.#exitAt) / LEAVE_DURATION, 1);

			return 1 - progress * progress * (3 - 2 * progress);
		}

		const progress = Math.max(0, Math.min((this.#time - this.#enteredAt) / ENTER_DURATION, 1));

		return this.#enterFrom + (1 - this.#enterFrom) * progress * progress * (3 - 2 * progress);
	}

	#schedule() {
		if (
			this.#disposed ||
			this.#frame ||
			!this.#running ||
			this.#reduced ||
			!this.#visible ||
			!this.#width ||
			!this.#height
		)
			return;

		this.#frame = requestAnimationFrame(this.#draw);
	}

	#pause() {
		cancelAnimationFrame(this.#frame);
		this.#frame = 0;
		this.#previous = 0;
	}

	#stop() {
		this.#pause();

		if (this.#running) {
			this.#running = false;
			this.#onRunning(false);
		}

		this.#context.clearRect(0, 0, this.#canvas.width, this.#canvas.height);
		this.#exitAt = null;
	}

	#prepare() {
		const context = this.#context;

		if (this.#paletteDirty) {
			const styles = getComputedStyle(this.#canvas);
			this.#baseSize = Number.parseFloat(styles.fontSize);
			this.#family = styles.fontFamily;
			context.font = `${this.#baseSize}px ${this.#family}`;
			this.#baseWidth = Math.ceil(context.measureText('M').width) + 1;
			this.#palette = [...this.#swatches.children].map((element) => {
				const style = getComputedStyle(element);

				return { background: style.backgroundColor, foreground: style.color };
			});
			this.#layoutDirty = true;
			this.#paletteDirty = false;
		}

		if (this.#layoutDirty) {
			this.#ratio = devicePixelRatio || 1;
			let fontSize =
				this.#baseSize *
				Math.max(
					1,
					Math.sqrt(
						(this.#width * this.#height) / (this.#baseWidth * this.#baseSize * 1.4 * MAX_CELLS)
					)
				);
			let font: string;

			do {
				font = `${fontSize}px ${this.#family}`;
				context.font = font;
				this.#pixelWidth = Math.ceil((context.measureText('M').width + 1) * this.#ratio);
				this.#pixelHeight = Math.ceil(fontSize * 1.4 * this.#ratio);
				this.#cellWidth = this.#pixelWidth / this.#ratio;
				this.#cellHeight = this.#pixelHeight / this.#ratio;
				this.#columns = Math.ceil(this.#width / this.#cellWidth);
				this.#rows = Math.ceil(this.#height / this.#cellHeight);
				fontSize *= 1.05;
			} while (this.#columns * this.#rows > MAX_CELLS);

			const width = Math.round(this.#width * this.#ratio);
			const height = Math.round(this.#height * this.#ratio);

			if (this.#canvas.width !== width) this.#canvas.width = width;
			if (this.#canvas.height !== height) this.#canvas.height = height;

			context.setTransform(this.#ratio, 0, 0, this.#ratio, 0, 0);
			context.imageSmoothingEnabled = false;

			const key = `${font}/${this.#ratio}/${this.#palette.map((tile) => `${tile.background}/${tile.foreground}`).join('/')}`;

			if (key !== this.#atlasKey) {
				this.#atlasKey = key;
				const glyphs = this.#glyphs;
				const cellWidth = this.#cellWidth;
				const cellHeight = this.#cellHeight;
				const ratio = this.#ratio;
				this.#atlas.width = this.#pixelWidth * CHARACTERS.length * RUN_LIMIT;
				this.#atlas.height = this.#pixelHeight * this.#palette.length;
				glyphs.setTransform(ratio, 0, 0, ratio, 0, 0);
				glyphs.font = font;
				glyphs.textBaseline = 'middle';

				for (let color = 0; color < this.#palette.length; color++) {
					glyphs.fillStyle = this.#palette[color].background;
					glyphs.fillRect(
						0,
						color * cellHeight,
						cellWidth * CHARACTERS.length * RUN_LIMIT,
						cellHeight
					);
					glyphs.fillStyle = this.#palette[color].foreground;

					for (let index = 1; index < CHARACTERS.length; index++) {
						for (let repeat = 0; repeat < RUN_LIMIT; repeat++) {
							glyphs.fillText(
								CHARACTERS[index],
								(index * RUN_LIMIT + repeat) * cellWidth,
								(color + 0.5) * cellHeight
							);
						}
					}
				}
			}

			if (this.#horizontal.length !== this.#rows || this.#vertical.length !== this.#columns) {
				this.#horizontal = new Float32Array(this.#rows);
				this.#vertical = new Float32Array(this.#columns);
				this.#radial = new Float32Array(this.#columns * this.#rows);
				this.#entryRanks = new Float32Array(this.#radial.length);
				this.#exitRanks = new Float32Array(this.#radial.length);

				for (let y = 0; y < this.#rows; y++) {
					for (let x = 0; x < this.#columns; x++) {
						this.#radial[y * this.#columns + x] =
							Math.hypot((x - this.#columns / 2) * 0.75, y - this.#rows / 2) * 0.22 * SINE_SCALE;
					}
				}

				this.#ranksDirty = true;
			}

			this.#layoutDirty = false;
		}

		if (this.#ranksDirty) {
			const origins = [this.#origin, this.#destination];
			const buffers = [this.#entryRanks, this.#exitRanks];

			for (let rank = 0; rank < origins.length; rank++) {
				const origin = origins[rank];
				const farthest = Math.hypot(
					Math.max(origin.x, 1 - origin.x),
					Math.max(origin.y, 1 - origin.y)
				);

				for (let y = 0; y < this.#rows; y++) {
					for (let x = 0; x < this.#columns; x++) {
						const distance = Math.hypot(
							(x + 0.5) / this.#columns - origin.x,
							(y + 0.5) / this.#rows - origin.y
						);
						buffers[rank][y * this.#columns + x] =
							(distance / farthest) * 0.9 + (((x * 17 + y * 31) % 23) / 23) * 0.1;
					}
				}
			}

			this.#ranksDirty = false;
		}
	}

	#blitRun(character: number, color: number, left: number, top: number, length: number) {
		const width = this.#cellWidth * length;
		const height = this.#cellHeight;

		this.#context.drawImage(
			this.#atlas,
			character * this.#pixelWidth * RUN_LIMIT,
			color * this.#pixelHeight,
			this.#pixelWidth * length,
			this.#pixelHeight,
			left,
			top,
			width,
			height
		);
	}

	#draw = (now: number) => {
		this.#frame = 0;

		if (!this.#width || !this.#height) {
			this.#pause();
			return;
		}

		if (this.#previous && now - this.#previous < FRAME_INTERVAL - 1) {
			this.#schedule();
			return;
		}

		this.#time += this.#previous ? Math.min(now - this.#previous, MAX_FRAME_STEP) : 0;
		this.#previous = now;

		if (this.#exitAt !== null && this.#time >= this.#exitAt + LEAVE_DURATION) {
			this.#stop();
			return;
		}

		if ((devicePixelRatio || 1) !== this.#deviceRatio) {
			this.#deviceRatio = devicePixelRatio || 1;
			this.#layoutDirty = true;
		}

		this.#prepare();

		const context = this.#context;
		const columns = this.#columns;
		const rows = this.#rows;
		const cellWidth = this.#cellWidth;
		const cellHeight = this.#cellHeight;
		const ratio = this.#ratio;
		const coverage = this.#coverage();
		const ranks =
			this.#returning || (this.#exitAt !== null && this.#time >= this.#exitAt)
				? this.#exitRanks
				: this.#entryRanks;
		const time = (this.#time * 0.0006 + this.#seed) * SINE_SCALE;

		context.clearRect(0, 0, this.#canvas.width / ratio, this.#canvas.height / ratio);

		for (let y = 0; y < rows; y++) {
			this.#horizontal[y] =
				SINE[((y * 0.08 * SINE_SCALE + time * 0.7) | 0) & SINE_MASK] * 2 * SINE_SCALE;
		}

		for (let x = 0; x < columns; x++) {
			this.#vertical[x] =
				SINE[((x * 0.06 * SINE_SCALE - time * 0.5 + SINE_SIZE / 4) | 0) & SINE_MASK] *
				2 *
				SINE_SCALE;
		}

		for (let y = 0, index = 0; y < rows; y++) {
			const rowPhase = y * 0.2 * SINE_SCALE;
			const rowOffset = this.#horizontal[y];
			const top = y * cellHeight;
			let length = 0;
			let runCharacter = -1;
			let runColor = -1;
			let left = 0;

			for (let x = 0; x < columns; x++, index++) {
				let reveal = (coverage * 1.3 - ranks[index]) / 0.3;

				if (reveal <= 0) {
					if (length) {
						this.#blitRun(runCharacter, runColor, left, top, length);
						length = 0;
					}

					continue;
				}

				reveal = Math.min(reveal, 1);
				const opened = reveal * reveal * (3 - 2 * reveal);
				const horizontal = x * 0.14 * SINE_SCALE + rowOffset;
				const vertical = rowPhase + this.#vertical[x];
				const wave =
					(SINE[((horizontal + time) | 0) & SINE_MASK] +
						SINE[((vertical - time * 0.8 + SINE_SIZE / 4) | 0) & SINE_MASK] +
						SINE[((this.#radial[index] - time * 1.2) | 0) & SINE_MASK]) /
					3;
				const character = Math.floor((wave + 1) * 0.5 * (CHARACTERS.length - 1));
				const colorWave = SINE[((horizontal * 0.6 + vertical * 0.4 + time * 0.55) | 0) & SINE_MASK];
				const color = Math.min(
					Math.floor((colorWave + 1) * 0.5 * this.#palette.length),
					this.#palette.length - 1
				);

				if (
					length &&
					(opened < 1 || character !== runCharacter || color !== runColor || length === RUN_LIMIT)
				) {
					this.#blitRun(runCharacter, runColor, left, top, length);
					length = 0;
				}

				if (opened === 1) {
					if (!length) {
						left = x * cellWidth;
						runCharacter = character;
						runColor = color;
					}

					length++;
					continue;
				}

				const pixels = Math.round(this.#pixelHeight * opened);

				if (!pixels) continue;

				const inset = Math.floor((this.#pixelHeight - pixels) / 2);

				context.drawImage(
					this.#atlas,
					character * this.#pixelWidth * RUN_LIMIT,
					color * this.#pixelHeight + inset,
					this.#pixelWidth,
					pixels,
					x * cellWidth,
					top + inset / ratio,
					cellWidth,
					pixels / ratio
				);
			}

			if (length) this.#blitRun(runCharacter, runColor, left, top, length);
		}

		this.#schedule();
	};
}
