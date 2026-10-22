import { Braces, Cpu, FileCode, FileType, Library, TriangleAlert } from '@lucide/svelte';
import type { EditorTab } from './editor-pane.svelte';
import type { Inspection, Inspector } from '#lib/playground/inspector';

const outputs = [
	{
		id: 'js_code',
		label: 'JS Bridge',
		language: 'typescript',
		filename: 'lib.rs.js',
		icon: FileCode
	},
	{ id: 'rust_code', label: 'Rust Bridge', language: 'rust', filename: 'bridge.rs', icon: Braces },
	{ id: 'ts_code', label: 'TS', language: 'typescript', filename: 'lib.rs.d.ts', icon: FileType },
	{
		id: 'ambient_ts_code',
		label: 'TS Ambient',
		language: 'typescript',
		filename: 'ambient.d.ts',
		icon: Library
	},
	{
		id: 'runtime_js_code',
		label: 'JS Runtime',
		language: 'typescript',
		filename: '$rust/unstable!/wasm',
		icon: Cpu
	}
] as const satisfies readonly (EditorTab & { id: keyof Inspection; filename: string })[];

const diagnosticsTab = {
	id: 'diagnostics',
	label: 'Diagnostics',
	language: 'diagnostics',
	filename: 'diagnostics.txt',
	icon: TriangleAlert
} as const satisfies EditorTab & { id: keyof Inspection; filename: string };

export function inspectionView(inspector: Inspector, selected: string) {
	const diagnostics = inspector.error || inspector.compilation?.diagnostics || '';
	const diagnosticCount = inspector.error ? 1 : (inspector.compilation?.diagnostic_count ?? 0);
	const tabs = diagnostics
		? [...outputs, { ...diagnosticsTab, label: `Diagnostics [${diagnosticCount}]` }]
		: outputs;
	const output = tabs.find((tab) => tab.id === selected) ?? outputs[0];
	const content = inspector.pending
		? '/* Generating... */'
		: output.id === 'diagnostics'
			? diagnostics
			: (inspector.compilation?.[output.id] ?? '');

	return {
		tabs,
		output,
		content,
		restart: !inspector.pending && output.id === 'diagnostics' && !!inspector.error,
		status: diagnostics
			? `${diagnosticCount} ${diagnosticCount === 1 ? 'diagnostic' : 'diagnostics'} available in the Diagnostics tab.`
			: ''
	};
}
