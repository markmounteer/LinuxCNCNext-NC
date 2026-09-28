"use strict";
// Only expected input-validation failures cross the reader/consumer boundary.
class ValidationError extends Error {
  constructor(message, context = {}) {
    super(message); this.name = "NextNCValidationError"; this.context = context;
  }
}
module.exports = {ValidationError};
