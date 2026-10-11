export interface EditorRange {
	start: number;
	end: number;
}

export interface EditorMarker extends EditorRange {
	severity: number;
	message: string;
	code: string;
}

export interface EditorHover extends EditorRange {
	markdown: string;
}

export interface LanguageService {
	hover(code: string, offset: number, signal: AbortSignal): Promise<EditorHover | undefined>;
	definition(code: string, offset: number, signal: AbortSignal): Promise<EditorRange | undefined>;
}
