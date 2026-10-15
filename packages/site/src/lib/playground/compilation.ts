import type {
	EditorHover,
	EditorMarker,
	EditorRange
} from '../ui/components/code-editor/language-service';

export interface Compilation {
	js_code: string;
	rust_code: string;
	ts_code: string;
	ambient_ts_code: string;
	ts_sourcemap: string;
	diagnostics: string;
	diagnostic_count: number;
	rust_markers: EditorMarker[];
}

export interface CompileRequest {
	id: number;
	code: string;
}

export type CompileResponse =
	{ id: number; compilation: Compilation } | { id: number; error: string; fatal: boolean };

export interface LanguageRequest {
	type: 'hover' | 'definition';
	id: number;
	code: string;
	offset: number;
}

export type LanguageResponse =
	| { type: 'hover'; id: number; hover?: EditorHover }
	| { type: 'definition'; id: number; definition?: EditorRange };

export type CompilerRequest = CompileRequest | LanguageRequest;
export type CompilerMessage = { type: 'ready' } | CompileResponse | LanguageResponse;
