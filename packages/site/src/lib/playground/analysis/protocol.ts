export type Analysis = {
	transformedJs: string;
	rustGlue: string;
	jsGlue: string;
	diagnostics: string;
};

export type AnalysisRequest = { id: number; code: string };
export type AnalysisResponse =
	| { type: 'ready' }
	| { type: 'result'; id: number; analysis: Analysis }
	| { type: 'error'; id: number; message: string; fatal: boolean };
