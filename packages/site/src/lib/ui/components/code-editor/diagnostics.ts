import { StreamLanguage } from '@codemirror/language';
import { Tag, tags } from '@lezer/highlight';

export const diagnosticTags = {
	code: Tag.define(),
	warning: Tag.define(),
	error: Tag.define(),
	location: Tag.define()
};

export const diagnosticsLanguage = StreamLanguage.define({
	tokenTable: {
		diagnosticCode: diagnosticTags.code,
		diagnosticWarning: diagnosticTags.warning,
		diagnosticError: diagnosticTags.error,
		diagnosticLocation: diagnosticTags.location,
		string: tags.string
	},
	token(stream) {
		if (stream.eatSpace()) return null;
		if (stream.match(/^(?:⚠|warning\b).*$/)) return 'diagnosticWarning';
		if (stream.match(/^(?:[×✖]|error\b).*$/)) return 'diagnosticError';
		if (stream.match(/^E\d{4}\b/)) return 'diagnosticError';
		if (stream.match(/^rustier::[\w:]+|^[\w-]+(?=: )/)) return 'diagnosticCode';
		if (
			stream.match(/^[│┃╭╰├└┌─━╮╯┼┬┴▶▲]+/) ||
			stream.match(/^\d+(?=\s*[│|])/) ||
			stream.match(/^\[(?:[^\]\n]*:)?\d+:\d+\]/)
		)
			return 'diagnosticLocation';
		if (stream.match(/^"(?:[^"\\]|\\.)*"/)) return 'string';
		stream.next();
		return null;
	}
});
