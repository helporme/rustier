export class RuntimeError extends globalThis.Error {
  constructor(message) {
    super(message)
    this.name = 'RuntimeError'
  }
}
