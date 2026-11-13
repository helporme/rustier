import { rs } from 'rustier';
import { transpileModule, ScriptTarget, ModuleKind } from 'typescript';
import type { AnalysisRequest, AnalysisResponse } from './protocol';

const workspace = rs`rustier_site::Workspace::new()`;

self.onmessage = ({ data }: MessageEvent<AnalysisRequest>) => {
	let response: AnalysisResponse;
	try {
		const result = rs`
			let workspace: &mut rustier_site::Workspace = ${workspace};
			let code: String = ${data.code};
			workspace.analyze(code)
		`;
		const transformed = rs`
			let result: &rustier_site::Analysis = ${result};
			result.transformed_js.clone()
		` as string;
		const diagnostics = rs`
			let result: &rustier_site::Analysis = ${result};
			result.diagnostics.clone()
		` as string;
		response = {
			type: 'result',
			id: data.id,
			analysis: {
				transformedJs: transpileModule(transformed, {
					fileName: 'index.ts',
					compilerOptions: {
						target: ScriptTarget.ESNext,
						module: ModuleKind.ESNext,
						verbatimModuleSyntax: true
					}
				}).outputText,
				rustGlue: rs`
					let result: &rustier_site::Analysis = ${result};
					result.rust_glue.clone()
				` as string,
				jsGlue: rs`
					let result: &rustier_site::Analysis = ${result};
					result.js_glue.clone()
				` as string,
				diagnostics
			}
		};
	} catch (cause) {
		response = {
			type: 'error',
			id: data.id,
			message: String(cause),
			fatal: cause instanceof WebAssembly.RuntimeError || cause instanceof RangeError
		};
	}
	self.postMessage(response);
	if (response.type === 'error' && response.fatal) self.close();
};

self.postMessage({ type: 'ready' } satisfies AnalysisResponse);
