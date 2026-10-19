export type Inspection = {
	js_code: string;
	rust_code: string;
	ts_code: string;
	ambient_ts_code: string;
	runtime_js_code: string;
	diagnostics: string;
	diagnostic_count: number;
};

export type InspectRequest = { id: number; code: string };

export type InspectResponse =
	| { type: 'ready' }
	| { type: 'result'; id: number; compilation: Inspection }
	| { type: 'error'; id: number; message: string; fatal: boolean };
