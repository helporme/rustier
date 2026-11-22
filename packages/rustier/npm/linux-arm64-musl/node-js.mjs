import{
	createRequire as __rustier_create_require
}from "node:module";
const __rustier_exports = __rustier_create_require(
	import.meta.url
)(
	"./bridge.node"
);
function __rustier_drop_handle(
	pointer
){
	let __rustier_drop_result;
	__rustier_drop_result= __rustier_exports.__rustier_drop_handle(pointer);
}
const __rustier_handle_key = Symbol();
const __rustier_handle_finalizer = new FinalizationRegistry(__rustier_drop_handle);
let __rustier_is_handle;
let __rustier_handle_pointer;

class Rs {
  #pointer;

  constructor(pointer, key) {
    if (key !== __rustier_handle_key) {
      throw new TypeError('Rust handles can only be created by Rust');
    }

    this.#pointer = pointer;
    __rustier_handle_finalizer.register(this, pointer);
  }

  static {
    __rustier_is_handle = value =>
      typeof value === 'object' && value !== null && #pointer in value;
    __rustier_handle_pointer = value => value.#pointer;
  }
}

export function __rustier_0702781657d797ac2692d1476652248b515287b53d301abbc86f78f593a16420(__rustier_arg_0, __rustier_arg_1, __rustier_arg_2){
	if(
		typeof __rustier_arg_0 !== "string"
	){
		throw new TypeError(
			"Rustier input 0: expected string"
		);
	}
	if(
		typeof __rustier_arg_1 !== "boolean"
	){
		throw new TypeError(
			"Rustier input 1: expected boolean"
		);
	}
	if(
		typeof __rustier_arg_2 !== "boolean"
	){
		throw new TypeError(
			"Rustier input 2: expected boolean"
		);
	}
	let __rustier_result;
	__rustier_result= __rustier_exports.__rustier_0702781657d797ac2692d1476652248b515287b53d301abbc86f78f593a16420(__rustier_arg_0, __rustier_arg_1, __rustier_arg_2);
	return new Rs(
		__rustier_result, __rustier_handle_key
	);
}

export function __rustier_272699e970adafcaa4fb5ec38d7dd462c1e119e7599d8ccb1beea84f72a95d34(__rustier_arg_0, __rustier_arg_1, __rustier_arg_2, __rustier_arg_3, __rustier_arg_4, __rustier_arg_5, __rustier_arg_6){
	if(
		!__rustier_is_handle(
			__rustier_arg_0
		)
	){
		throw new TypeError(
			"Rustier input 0: expected Rust handle"
		);
	}
	if(
		!__rustier_is_handle(
			__rustier_arg_1
		)
	){
		throw new TypeError(
			"Rustier input 1: expected Rust handle"
		);
	}
	if(
		!__rustier_is_handle(
			__rustier_arg_2
		)
	){
		throw new TypeError(
			"Rustier input 2: expected Rust handle"
		);
	}
	if(
		typeof __rustier_arg_3 !== "string"
	){
		throw new TypeError(
			"Rustier input 3: expected string"
		);
	}
	if(
		typeof __rustier_arg_4 !== "string"
	){
		throw new TypeError(
			"Rustier input 4: expected string"
		);
	}
	if(
		typeof __rustier_arg_5 !== "string"
	){
		throw new TypeError(
			"Rustier input 5: expected string"
		);
	}
	if(
		!__rustier_is_handle(
			__rustier_arg_6
		)
	){
		throw new TypeError(
			"Rustier input 6: expected Rust handle"
		);
	}
	let __rustier_result;
	try{
		__rustier_result= __rustier_exports.__rustier_272699e970adafcaa4fb5ec38d7dd462c1e119e7599d8ccb1beea84f72a95d34(__rustier_handle_pointer(
			__rustier_arg_0
		), __rustier_handle_pointer(
			__rustier_arg_1
		), __rustier_handle_pointer(
			__rustier_arg_2
		), __rustier_arg_3, __rustier_arg_4, __rustier_arg_5, __rustier_handle_pointer(
			__rustier_arg_6
		));
	}catch(
		error
	){
		if(
			error instanceof Error && error.name === "RustError" && typeof
			Error.captureStackTrace === "function"
		){
			Error.captureStackTrace(
				error,__rustier_272699e970adafcaa4fb5ec38d7dd462c1e119e7599d8ccb1beea84f72a95d34
			);
		}
		throw error;
	}
	return undefined;
}

export function __rustier_2c6b2fca332b927a03ebfff52e29ed7d775e0e1a7be47a9e5c97031a9af866b0(__rustier_arg_0){
	if(
		typeof __rustier_arg_0 !== "string"
	){
		throw new TypeError(
			"Rustier input 0: expected string"
		);
	}
	let __rustier_result;
	try{
		__rustier_result= __rustier_exports.__rustier_2c6b2fca332b927a03ebfff52e29ed7d775e0e1a7be47a9e5c97031a9af866b0(__rustier_arg_0);
	}catch(
		error
	){
		if(
			error instanceof Error && error.name === "RustError" && typeof
			Error.captureStackTrace === "function"
		){
			Error.captureStackTrace(
				error,__rustier_2c6b2fca332b927a03ebfff52e29ed7d775e0e1a7be47a9e5c97031a9af866b0
			);
		}
		throw error;
	}
	return new Rs(
		__rustier_result, __rustier_handle_key
	);
}

export function __rustier_7091187569c55db4cf8237718b2f05f9f7189551c86149d3c8cbad6f31edd743(__rustier_arg_0, __rustier_arg_1){
	if(
		!__rustier_is_handle(
			__rustier_arg_0
		)
	){
		throw new TypeError(
			"Rustier input 0: expected Rust handle"
		);
	}
	if(
		!__rustier_is_handle(
			__rustier_arg_1
		)
	){
		throw new TypeError(
			"Rustier input 1: expected Rust handle"
		);
	}
	let __rustier_result;
	try{
		__rustier_result= __rustier_exports.__rustier_7091187569c55db4cf8237718b2f05f9f7189551c86149d3c8cbad6f31edd743(__rustier_handle_pointer(
			__rustier_arg_0
		), __rustier_handle_pointer(
			__rustier_arg_1
		));
	}catch(
		error
	){
		if(
			error instanceof Error && error.name === "RustError" && typeof
			Error.captureStackTrace === "function"
		){
			Error.captureStackTrace(
				error,__rustier_7091187569c55db4cf8237718b2f05f9f7189551c86149d3c8cbad6f31edd743
			);
		}
		throw error;
	}
	return __rustier_result;
}

export function __rustier_728f0c3a305e88e2aeb8cbdb59a33e930bce223e383895afff229191ce7ddc59(__rustier_arg_0, __rustier_arg_1, __rustier_arg_2){
	if(
		!__rustier_is_handle(
			__rustier_arg_0
		)
	){
		throw new TypeError(
			"Rustier input 0: expected Rust handle"
		);
	}
	if(
		typeof __rustier_arg_1 !== "string"
	){
		throw new TypeError(
			"Rustier input 1: expected string"
		);
	}
	if(
		!__rustier_is_handle(
			__rustier_arg_2
		)
	){
		throw new TypeError(
			"Rustier input 2: expected Rust handle"
		);
	}
	let __rustier_result;
	try{
		__rustier_result= __rustier_exports.__rustier_728f0c3a305e88e2aeb8cbdb59a33e930bce223e383895afff229191ce7ddc59(__rustier_handle_pointer(
			__rustier_arg_0
		), __rustier_arg_1, __rustier_handle_pointer(
			__rustier_arg_2
		));
	}catch(
		error
	){
		if(
			error instanceof Error && error.name === "RustError" && typeof
			Error.captureStackTrace === "function"
		){
			Error.captureStackTrace(
				error,__rustier_728f0c3a305e88e2aeb8cbdb59a33e930bce223e383895afff229191ce7ddc59
			);
		}
		throw error;
	}
	return new Rs(
		__rustier_result, __rustier_handle_key
	);
}

export function __rustier_ab85e2fa2ef1c04e81b07bffb31aad23bc53ddcec19d69e4b89dcfbae70f06ef(__rustier_arg_0, __rustier_arg_1){
	if(
		typeof __rustier_arg_0 !== "string"
	){
		throw new TypeError(
			"Rustier input 0: expected string"
		);
	}
	if(
		typeof __rustier_arg_1 !== "number"
	){
		throw new TypeError(
			"Rustier input 1: expected number"
		);
	}
	let __rustier_result;
	try{
		__rustier_result= __rustier_exports.__rustier_ab85e2fa2ef1c04e81b07bffb31aad23bc53ddcec19d69e4b89dcfbae70f06ef(__rustier_arg_0, __rustier_arg_1);
	}catch(
		error
	){
		if(
			error instanceof Error && error.name === "RustError" && typeof
			Error.captureStackTrace === "function"
		){
			Error.captureStackTrace(
				error,__rustier_ab85e2fa2ef1c04e81b07bffb31aad23bc53ddcec19d69e4b89dcfbae70f06ef
			);
		}
		throw error;
	}
	return undefined;
}

export function __rustier_b8d191727b94c0932759afd56243cb164076010f1224e6ef41c97c22f7e12342(){
	let __rustier_result;
	__rustier_result= __rustier_exports.__rustier_b8d191727b94c0932759afd56243cb164076010f1224e6ef41c97c22f7e12342();
	return new Rs(
		__rustier_result, __rustier_handle_key
	);
}

export function __rustier_bb3f9615872f4a80a48b4d60c902483fac7b4bb6d79e3566d7f1eb6f231dfe93(__rustier_arg_0){
	if(
		typeof __rustier_arg_0 !== "string"
	){
		throw new TypeError(
			"Rustier input 0: expected string"
		);
	}
	let __rustier_result;
	try{
		__rustier_result= __rustier_exports.__rustier_bb3f9615872f4a80a48b4d60c902483fac7b4bb6d79e3566d7f1eb6f231dfe93(__rustier_arg_0);
	}catch(
		error
	){
		if(
			error instanceof Error && error.name === "RustError" && typeof
			Error.captureStackTrace === "function"
		){
			Error.captureStackTrace(
				error,__rustier_bb3f9615872f4a80a48b4d60c902483fac7b4bb6d79e3566d7f1eb6f231dfe93
			);
		}
		throw error;
	}
	return undefined;
}

export function __rustier_ccb66c37e8581afcbdbee8ba550b867999e8456e294a77b1d1ee792181541d9b(__rustier_arg_0, __rustier_arg_1, __rustier_arg_2, __rustier_arg_3){
	if(
		!__rustier_is_handle(
			__rustier_arg_0
		)
	){
		throw new TypeError(
			"Rustier input 0: expected Rust handle"
		);
	}
	if(
		!__rustier_is_handle(
			__rustier_arg_1
		)
	){
		throw new TypeError(
			"Rustier input 1: expected Rust handle"
		);
	}
	if(
		typeof __rustier_arg_2 !== "string"
	){
		throw new TypeError(
			"Rustier input 2: expected string"
		);
	}
	if(
		!__rustier_is_handle(
			__rustier_arg_3
		)
	){
		throw new TypeError(
			"Rustier input 3: expected Rust handle"
		);
	}
	let __rustier_result;
	try{
		__rustier_result= __rustier_exports.__rustier_ccb66c37e8581afcbdbee8ba550b867999e8456e294a77b1d1ee792181541d9b(__rustier_handle_pointer(
			__rustier_arg_0
		), __rustier_handle_pointer(
			__rustier_arg_1
		), __rustier_arg_2, __rustier_handle_pointer(
			__rustier_arg_3
		));
	}catch(
		error
	){
		if(
			error instanceof Error && error.name === "RustError" && typeof
			Error.captureStackTrace === "function"
		){
			Error.captureStackTrace(
				error,__rustier_ccb66c37e8581afcbdbee8ba550b867999e8456e294a77b1d1ee792181541d9b
			);
		}
		throw error;
	}
	return __rustier_result;
}

export function __rustier_e5fd93ef8335e08dcb72959b249adc12d18918757e7dff4703347cfeae18b86a(__rustier_arg_0, __rustier_arg_1, __rustier_arg_2, __rustier_arg_3, __rustier_arg_4){
	if(
		!__rustier_is_handle(
			__rustier_arg_0
		)
	){
		throw new TypeError(
			"Rustier input 0: expected Rust handle"
		);
	}
	if(
		!__rustier_is_handle(
			__rustier_arg_1
		)
	){
		throw new TypeError(
			"Rustier input 1: expected Rust handle"
		);
	}
	if(
		typeof __rustier_arg_2 !== "string"
	){
		throw new TypeError(
			"Rustier input 2: expected string"
		);
	}
	if(
		typeof __rustier_arg_3 !== "string"
	){
		throw new TypeError(
			"Rustier input 3: expected string"
		);
	}
	if(
		typeof __rustier_arg_4 !== "string"
	){
		throw new TypeError(
			"Rustier input 4: expected string"
		);
	}
	let __rustier_result;
	try{
		__rustier_result= __rustier_exports.__rustier_e5fd93ef8335e08dcb72959b249adc12d18918757e7dff4703347cfeae18b86a(__rustier_handle_pointer(
			__rustier_arg_0
		), __rustier_handle_pointer(
			__rustier_arg_1
		), __rustier_arg_2, __rustier_arg_3, __rustier_arg_4);
	}catch(
		error
	){
		if(
			error instanceof Error && error.name === "RustError" && typeof
			Error.captureStackTrace === "function"
		){
			Error.captureStackTrace(
				error,__rustier_e5fd93ef8335e08dcb72959b249adc12d18918757e7dff4703347cfeae18b86a
			);
		}
		throw error;
	}
	return __rustier_result;
}

export function __rustier_efc3221a8e3681e5aae648616061ad3b51ba9c27be5f3939741856db7c5cdfcc(__rustier_arg_0, __rustier_arg_1, __rustier_arg_2){
	if(
		!__rustier_is_handle(
			__rustier_arg_0
		)
	){
		throw new TypeError(
			"Rustier input 0: expected Rust handle"
		);
	}
	if(
		typeof __rustier_arg_1 !== "number"
	){
		throw new TypeError(
			"Rustier input 1: expected number"
		);
	}
	if(
		typeof __rustier_arg_2 !== "number"
	){
		throw new TypeError(
			"Rustier input 2: expected number"
		);
	}
	let __rustier_result;
	try{
		__rustier_result= __rustier_exports.__rustier_efc3221a8e3681e5aae648616061ad3b51ba9c27be5f3939741856db7c5cdfcc(__rustier_handle_pointer(
			__rustier_arg_0
		), __rustier_arg_1, __rustier_arg_2);
	}catch(
		error
	){
		if(
			error instanceof Error && error.name === "RustError" && typeof
			Error.captureStackTrace === "function"
		){
			Error.captureStackTrace(
				error,__rustier_efc3221a8e3681e5aae648616061ad3b51ba9c27be5f3939741856db7c5cdfcc
			);
		}
		throw error;
	}
	return __rustier_result;
}

export function __rustier_fd9defce4c42fe9c8cafce27c536719a60c77420ca15556d59300da4d669d100(){
	let __rustier_result;
	try{
		__rustier_result= __rustier_exports.__rustier_fd9defce4c42fe9c8cafce27c536719a60c77420ca15556d59300da4d669d100();
	}catch(
		error
	){
		if(
			error instanceof Error && error.name === "RustError" && typeof
			Error.captureStackTrace === "function"
		){
			Error.captureStackTrace(
				error,__rustier_fd9defce4c42fe9c8cafce27c536719a60c77420ca15556d59300da4d669d100
			);
		}
		throw error;
	}
	return __rustier_result;
}

export function __rustier_fffe047fb26689589b00cc5dce55d16d3b73a904371e2c7ab06d391329f9684d(){
	let __rustier_result;
	__rustier_result= __rustier_exports.__rustier_fffe047fb26689589b00cc5dce55d16d3b73a904371e2c7ab06d391329f9684d();
	return new Rs(
		__rustier_result, __rustier_handle_key
	);
}