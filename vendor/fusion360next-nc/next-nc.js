/* SPDX-License-Identifier: MIT
 * Original AP238 toolpath writer. ES5 syntax also runs inside Autodesk's post engine.
 */
var NextNC = (function () {
  "use strict";
  var VERSION = "0.2.0";
  var PROFILE = "next-nc/turning-toolpath/0.1";
  var MILL_PROFILE = "next-nc/milling-toolpath/0.1";
  var planes = {XY: {normal: [0, 0, 1], axes: [0, 1], fixed: 2}, XZ: {normal: [0, 1, 0], axes: [0, 2], fixed: 1}, YZ: {normal: [1, 0, 0], axes: [1, 2], fixed: 0}};
  function requireValue(ok, message) { if (!ok) { throw new Error("Next-NC: " + message); } }
  function finite(n, label) {
    requireValue(typeof n === "number" && isFinite(n) && Math.abs(n) < 1e15, label + " must be a finite number below 1e15 in magnitude");
    return n;
  }
  function positive(n, label) { finite(n, label); requireValue(n > 0, label + " must be positive"); return n; }
  function integer(n, label, minimum) {
    finite(n, label); requireValue(n === Math.floor(n) && n >= minimum, label + " must be an integer >= " + minimum); return n;
  }
  function point(p, machine) {
    requireValue(p && p.length === 3, "a point must have X, Y, Z coordinates");
    var q = [finite(p[0], "X"), finite(p[1], "Y"), finite(p[2], "Z")];
    if (machine !== "mill") { requireValue(Math.abs(q[1]) <= 1e-9, "only XZ turning with Y=0 is supported"); q[1] = 0; } return q;
  }
  function same(a, b) { return a[0] === b[0] && a[1] === b[1] && a[2] === b[2]; }
  function distance(a, b) { return Math.sqrt(Math.pow(a[0] - b[0], 2) + Math.pow(a[1] - b[1], 2) + Math.pow(a[2] - b[2], 2)); }
  function copySpindle(s) {
    requireValue(s && (s.mode === "rpm" || s.mode === "css"), "spindle mode must be rpm or css");
    requireValue(typeof s.clockwise === "boolean", "spindle direction must be explicit");
    var result = {mode: s.mode, speed: positive(s.speed, "spindle speed"), clockwise: s.clockwise};
    if (s.mode === "css") { result.maximumRPM = positive(s.maximumRPM, "CSS maximum RPM"); }
    return result;
  }
  function coolant(c) { requireValue(["off", "flood", "mist", "through tool"].indexOf(c) >= 0, "unsupported coolant: " + c); return c; }
  function feed(f) {
    requireValue(f && (f.mode === "perMinute" || f.mode === "perRevolution"), "feed mode must be perMinute or perRevolution");
    return {value: positive(f.value, "feed"), mode: f.mode};
  }
  function stateKey(s, f) {
    return [s.mode, s.speed, s.maximumRPM || 0, s.clockwise, f ? f.mode : "", f ? f.value : ""].join("|");
  }
  function Section(spec, machine) {
    this.machine = machine;
    requireValue(spec && spec.tool, "section and tool are required");
    this.name = String(spec.name || (machine === "mill" ? "Milling" : "Turning"));
    this.tool = {number: integer(spec.tool.number, "tool number", 1),
      offset: integer(spec.tool.offset, "tool offset", 0), description: String(spec.tool.description || "")};
    this.workOffset = integer(spec.workOffset, "Fusion work offset", 0);
    this.start = point(spec.start, machine);
    this.position = this.start.slice();
    this.setSpindle(spec.spindle);
    this.coolant = coolant(spec.coolant);
    this.initialSpindle = copySpindle(spec.spindle);
    this.initialCoolant = this.coolant;
    this.paths = [];
  }
  Section.prototype.setSpindle = function (s) {
    var spindle = copySpindle(s); requireValue(this.machine !== "mill" || spindle.mode === "rpm", "XYZ milling requires constant RPM"); this.spindle = spindle;
  };
  Section.prototype.setCoolant = function (c) { this.coolant = coolant(c); };
  Section.prototype.append = function (path, f) {
    path.spindle = copySpindle(this.spindle); path.coolant = this.coolant;
    path.feed = f ? feed(f) : null;
    path.key = path.kind + "|" + path.coolant + "|" + stateKey(path.spindle, path.feed);
    var previous = this.paths[this.paths.length - 1];
    // Compact only consecutive linear moves with identical process state.
    // Every intermediate vertex is preserved; this is not geometric smoothing.
    if ((path.kind === "rapid" || path.kind === "linear") && previous && previous.key === path.key) {
      previous.points.push(path.points[1]);
    } else { this.paths.push(path); }
  };
  Section.prototype.rapid = function (end) {
    end = point(end, this.machine);
    if (!same(this.position, end)) { this.append({kind: "rapid", points: [this.position, end]}); }
    this.position = end;
  };
  Section.prototype.linear = function (end, f) {
    end = point(end, this.machine); f = feed(f);
    if (!same(this.position, end)) { this.append({kind: "linear", points: [this.position, end]}, f); }
    this.position = end;
  };
  Section.prototype.arc = function (end, center, clockwise, f, fullCircle, plane) {
    end = point(end, this.machine); center = point(center, this.machine); f = feed(f);
    plane = plane || (this.machine === "mill" ? "XY" : "XZ");
    requireValue(Object.prototype.hasOwnProperty.call(planes, plane) && (this.machine === "mill" || plane === "XZ"), "unsupported arc plane");
    var frame = planes[plane];
    requireValue(this.position[frame.fixed] === end[frame.fixed] && center[frame.fixed] === end[frame.fixed], "arc must be planar; linearize helices in CAM");
    requireValue(typeof clockwise === "boolean", "arc direction must be explicit");
    var radius = positive(distance(this.position, center), "arc radius");
    var radialDifference = Math.abs(distance(end, center) - radius);
    if (radialDifference > Math.max(1e-7, radius * 1e-6)) {
      var arcError = new Error("Next-NC: arc endpoints have different radii");
      arcError.code = "ARC_RADII"; arcError.radialDifference = radialDifference;
      throw arcError;
    }
    requireValue(fullCircle ? same(this.position, end) : !same(this.position, end), "full-circle flag must match arc endpoints");
    this.append({kind: "arc", start: this.position, end: end, center: center, radius: radius,
      clockwise: clockwise, fullCircle: !!fullCircle, plane: plane}, f);
    this.position = end;
  };
  Section.prototype.dwell = function (seconds) { this.append({kind: "dwell", seconds: positive(seconds, "dwell")}); };
  function Program(options) {
    options = options || {};
    requireValue(options.units === "mm" || options.units === "inch", "units must be mm or inch");
    this.machine = options.machine || "lathe";
    requireValue(this.machine === "lathe" || this.machine === "mill", "machine must be lathe or mill");
    this.profile = this.machine === "mill" ? MILL_PROFILE : PROFILE;
    this.units = options.units; this.name = String(options.name || "Next-NC program");
    this.timestamp = options.timestamp || new Date().toISOString();
    requireValue(/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?Z$/.test(this.timestamp), "timestamp must be UTC ISO 8601");
    this.sections = [];
  }
  Program.prototype.addSection = function (spec) { var s = new Section(spec, this.machine); this.sections.push(s); return s; };
  function real(n) {
    finite(n, "STEP real");
    var result = (n === 0 ? "0" : String(n)).toUpperCase();
    if (result.indexOf("E") >= 0) {
      var parts = result.split("E"); if (parts[0].indexOf(".") < 0) { parts[0] += "."; }
      return parts[0] + "E" + parts[1];
    }
    return result.indexOf(".") >= 0 ? result : result + ".";
  }
  function str(value) {
    var s = String(value), result = "'", hex, code, i;
    for (i = 0; i < s.length; ++i) {
      code = s.charCodeAt(i);
      if (s[i] === "'") { result += "''"; }
      else if (s[i] === "\\") { result += "\\\\"; }
      else if (code >= 32 && code <= 126) { result += s[i]; }
      else {
        // Part 21 X2 encodes UTF-16 code units, including surrogate pairs.
        hex = code.toString(16).toUpperCase();
        if (code >= 0xD800 && code <= 0xDBFF) {
          var low = s.charCodeAt(++i);
          requireValue(low >= 0xDC00 && low <= 0xDFFF, "unpaired Unicode surrogate");
          hex += ("0000" + low.toString(16).toUpperCase()).slice(-4);
        } else { requireValue(code < 0xDC00 || code > 0xDFFF, "unpaired Unicode surrogate"); }
        result += "\\X2\\" + (hex.length < 4 ? ("0000" + hex).slice(-4) : hex) + "\\X0\\";
      }
    }
    return result + "'";
  }
  function list(items) { return "(" + items.join(",") + ")"; }
  // These records are immutable values. Never intern actions, paths, properties,
  // relationships or operations: equal fields do not imply equal execution.
  var sharedTypes = {
    CARTESIAN_POINT: true, DIRECTION: true, DESCRIPTIVE_REPRESENTATION_ITEM: true,
    MEASURE_REPRESENTATION_ITEM: true, REPRESENTATION: true, ACTION_RESOURCE_TYPE: true,
    MACHINING_FEED_SPEED_REPRESENTATION: true, MACHINING_SPINDLE_SPEED_REPRESENTATION: true,
    MACHINING_TOOLPATH_SPEED_PROFILE_REPRESENTATION: true, DERIVED_UNIT_ELEMENT: true,
    POLYLINE: true, AXIS2_PLACEMENT_3D: true, CIRCLE: true, TRIMMED_CURVE: true
  };
  function Writer(machine) {
    this.machine = machine; this.process = machine === "mill" ? "milling" : "turning";
    this.lines = []; this.valueCache = Object.create(null); this.reused = 0;
    this.curves = {polylines: 0, arcs: 0};
  }
  Writer.prototype.raw = function (value) { this.lines.push(value); return "#" + this.lines.length; };
  Writer.prototype.add = function (type, args) {
    var record = type + list(args);
    if (!Object.prototype.hasOwnProperty.call(sharedTypes, type)) { return this.raw(record); }
    if (this.valueCache[record]) { ++this.reused; return this.valueCache[record]; }
    if (type === "POLYLINE") { ++this.curves.polylines; }
    else if (type === "TRIMMED_CURVE") { ++this.curves.arcs; }
    var reference = this.raw(record); this.valueCache[record] = reference; return reference;
  };
  Writer.prototype.textItem = function (name, value) { return this.add("DESCRIPTIVE_REPRESENTATION_ITEM", [str(name), str(value)]); };
  Writer.prototype.point = function (p) { return this.add("CARTESIAN_POINT", ["''", list(p.map(real))]); };
  Writer.prototype.rep = function (type, name, items, context) { return this.add(type || "REPRESENTATION", [str(name), list(items), context || this.unitless]); };
  Writer.prototype.property = function (target, name, rep) {
    var prop = this.add("ACTION_PROPERTY", [str(name), "''", target]);
    this.add("ACTION_PROPERTY_REPRESENTATION", ["''", "''", prop, rep]);
  };
  Writer.prototype.textProperty = function (target, name, value) {
    this.property(target, name, this.rep(null, "", [this.textItem(name, value)]));
  };
  Writer.prototype.measure = function (name, value, unit) {
    return this.add("MEASURE_REPRESENTATION_ITEM", [str(name), "NUMERIC_MEASURE(" + real(value) + ")", unit]);
  };
  Writer.prototype.rel = function (kind, a, b, order) {
    var args = ["''", "''", a, b]; if (order !== undefined) { args.push(real(order)); }
    return this.add(kind, args);
  };
  Writer.prototype.method = function (type, name, description) { return this.add(type, [str(name), str(description || ""), "''", "''"]); };
  Writer.prototype.units = function (units) {
    this.unitless = this.add("REPRESENTATION_CONTEXT", ["''", str("units not necessary")]);
    this.length = this.raw("(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.))");
    if (units === "inch") {
      var factor = this.add("LENGTH_MEASURE_WITH_UNIT", ["LENGTH_MEASURE(25.4)", this.length]);
      var dims = this.add("DIMENSIONAL_EXPONENTS", ["1.", "0.", "0.", "0.", "0.", "0.", "0."]);
      this.length = this.raw("(CONVERSION_BASED_UNIT('inch'," + factor + ") LENGTH_UNIT() NAMED_UNIT(" + dims + "))");
    }
    var angle = this.raw("(NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.))");
    var solid = this.raw("(NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT())");
    this.context = this.raw("(GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNIT_ASSIGNED_CONTEXT(" + list([this.length, angle, solid]) + ") REPRESENTATION_CONTEXT('','3D'))");
    this.second = this.raw("(NAMED_UNIT(*) SI_UNIT($,.SECOND.) TIME_UNIT())");
    var minuteFactor = this.add("TIME_MEASURE_WITH_UNIT", ["TIME_MEASURE(60.)", this.second]);
    var timeDims = this.add("DIMENSIONAL_EXPONENTS", ["0.", "0.", "1.", "0.", "0.", "0.", "0."]);
    var minute = this.raw("(CONVERSION_BASED_UNIT('minute'," + minuteFactor + ") NAMED_UNIT(" + timeDims + ") TIME_UNIT())");
    var zeroDims = this.add("DIMENSIONAL_EXPONENTS", ["0.", "0.", "0.", "0.", "0.", "0.", "0."]);
    var rev = this.add("CONTEXT_DEPENDENT_UNIT", [zeroDims, str("revolution")]);
    var self = this;
    function divided(a, b) {
      return self.add("DERIVED_UNIT", [list([self.add("DERIVED_UNIT_ELEMENT", [a, "1."]), self.add("DERIVED_UNIT_ELEMENT", [b, "-1."])])]);
    }
    this.rpm = divided(rev, minute); this.perMinute = divided(this.length, minute); this.perRevolution = divided(this.length, rev);
    this.technologyCache = {}; this.functionsCache = {};
  };
  Writer.prototype.technology = function (spindle, f) {
    var key = stateKey(spindle, f), cached = this.technologyCache[key]; if (cached) { return cached; }
    var tech = this.method("MACHINING_TECHNOLOGY", "", this.process), items;
    // AP238 uses a right-handed sign: clockwise is negative (opposite Fusion).
    var signed = spindle.speed * (spindle.clockwise ? -1 : 1);
    if (spindle.mode === "css") {
      items = [this.measure("surface speed", signed, this.perMinute), this.measure("maximum rotational speed", spindle.maximumRPM, this.rpm)];
    } else { items = [this.measure("rotational speed", signed, this.rpm)]; }
    this.property(tech, "spindle", this.rep("MACHINING_SPINDLE_SPEED_REPRESENTATION", spindle.mode === "css" ? "cutting speed" : "spindle speed", items));
    if (f) {
      var name = f.mode === "perMinute" ? "feed speed" : "feed per revolution";
      this.property(tech, "feedrate", this.rep("MACHINING_FEED_SPEED_REPRESENTATION", name, [this.measure(name, f.value, this[f.mode])]));
    }
    this.textProperty(tech, "feedrate reference", "tool center point");
    this.technologyCache[key] = tech; return tech;
  };
  Writer.prototype.functions = function (coolantMode) {
    var cached = this.functionsCache[coolantMode]; if (cached) { return cached; }
    var functions = this.method("MACHINING_FUNCTIONS", "", this.process);
    this.textProperty(functions, "coolant", coolantMode === "off" ? "coolant off" : "coolant on");
    if (coolantMode !== "off") { this.textProperty(functions, "coolant type", coolantMode); }
    this.functionsCache[coolantMode] = functions; return functions;
  };
  Writer.prototype.curve = function (path) {
    if (path.kind !== "arc") { return this.add("POLYLINE", ["''", list(path.points.map(this.point.bind(this)))]); }
    var normal = this.add("DIRECTION", ["''", list(planes[path.plane].normal.map(real))]);
    var direction = path.start.map(function (v, i) { return (v - path.center[i]) / path.radius; });
    var ref = this.add("DIRECTION", ["''", list(direction.map(real))]);
    var placement = this.add("AXIS2_PLACEMENT_3D", ["''", this.point(path.center), normal, ref]);
    var circle = this.add("CIRCLE", ["''", placement, real(path.radius)]);
    // +Y is the G18/ZX plane normal; counterclockwise is right-handed about +Y.
    var first = path.fullCircle ? "(PARAMETER_VALUE(0.))" : list([this.point(path.start)]);
    var last = path.fullCircle ? "(PARAMETER_VALUE(" + real(2 * Math.PI) + "))" : list([this.point(path.end)]);
    return this.add("TRIMMED_CURVE", ["''", circle, first, last, path.clockwise ? ".F." : ".T.", path.fullCircle ? ".PARAMETER." : ".CARTESIAN."]);
  };
  Writer.prototype.toolpath = function (operation, path, index) {
    var tp = this.method("MACHINING_TOOLPATH", "Path " + index, path.kind === "dwell" ? "feedstop" : "cutter location trajectory");
    this.rel("MACHINING_TOOLPATH_SEQUENCE_RELATIONSHIP", operation, tp, index);
    this.rel("MACHINING_TECHNOLOGY_RELATIONSHIP", tp, this.technology(path.spindle, path.feed));
    this.rel("MACHINING_FUNCTIONS_RELATIONSHIP", tp, this.functions(path.coolant));
    this.textProperty(tp, "priority", "required");
    if (path.kind === "dwell") {
      var time = this.add("MEASURE_REPRESENTATION_ITEM", [str("dwell"), "TIME_MEASURE(" + real(path.seconds) + ")", this.second]);
      this.property(tp, "dwell", this.rep(null, "", [time]));
    } else {
      this.textProperty(tp, "trajectory type", "trajectory path"); this.textProperty(tp, "direction", "beginning to end");
      this.property(tp, "basic curve", this.rep(null, "", [this.curve(path)], this.context));
      if (path.kind === "rapid") {
        this.property(tp, "speed profile", this.rep("MACHINING_TOOLPATH_SPEED_PROFILE_REPRESENTATION", "", [this.textItem("", "rapid")]));
      }
    }
  };
  Writer.prototype.project = function (program) {
    var app = this.add("APPLICATION_CONTEXT", [str("Application protocol for the exchange of CNC data")]);
    this.add("APPLICATION_PROTOCOL_DEFINITION", [str("international standard"), str("integrated_cnc_schema"), "2007", app]);
    var context = this.add("PRODUCT_CONTEXT", [str("CNC Machining"), app, str("manufacturing")]);
    var defContext = this.add("PRODUCT_DEFINITION_CONTEXT", [str("CNC Machining"), app, str("manufacturing")]);
    var project = this.add("MACHINING_PROJECT", [str(program.name), str(program.name), "$", list([context])]);
    var formation = this.add("PRODUCT_DEFINITION_FORMATION", [str(VERSION), "''", project]);
    var projectDef = this.add("PRODUCT_DEFINITION", ["''", "''", formation, defContext]);
    var workplan = this.method("MACHINING_WORKPLAN", program.name, "");
    var process = this.add("PRODUCT_DEFINITION_PROCESS", [str("machining"), "''", workplan, "''"]);
    this.add("PROCESS_PRODUCT_ASSOCIATION", ["''", "''", projectDef, process]);
    this.textProperty(workplan, "next-nc profile", program.profile);
    this.textProperty(workplan, "next-nc coordinates", program.machine === "mill" ? "WCS; XYZ Cartesian; fixed +Z tool axis; Fusion tool reference point" : "WCS; X radius; Y zero; Z axial; Fusion tool reference point");
    var part = this.add("PRODUCT", [str("workpiece"), str("Toolpath-only workpiece; geometry not supplied"), "$", list([context])]);
    var partFormation = this.add("PRODUCT_DEFINITION_FORMATION", ["''", "''", part]);
    var partDef = this.add("PRODUCT_DEFINITION", [str("workpiece"), "''", partFormation, defContext]);
    this.add("MACHINING_PROJECT_WORKPIECE_RELATIONSHIP", ["''", str("workpiece"), "''", projectDef, partDef]);
    this.partShape = this.add("PRODUCT_DEFINITION_SHAPE", ["''", "''", partDef]);
    return workplan;
  };
  Writer.prototype.section = function (workplan, section, index) {
    requireValue(section.paths.length > 0, "section '" + section.name + "' contains no motion or dwell");
    var ws = this.method("MACHINING_WORKINGSTEP", section.name, "machining");
    this.rel("MACHINING_PROCESS_SEQUENCE_RELATIONSHIP", workplan, ws, index);
    var operation = this.method(this.machine === "mill" ? "MILLING_TYPE_OPERATION" : "TURNING_TYPE_OPERATION", section.name, "");
    this.rel("MACHINING_OPERATION_RELATIONSHIP", ws, operation);
    // A generic CC1 toolpath feature avoids claiming feature-based turning geometry.
    var feature = this.add("INSTANCED_FEATURE", ["''", str("toolpath"), "''", str("toolpath"), this.partShape, ".T."]);
    var featureProcess = this.method("MACHINING_FEATURE_PROCESS", "", "machining");
    var process = this.add("PROPERTY_PROCESS", [str("machining"), "''", featureProcess, "''"]);
    this.add("PROCESS_PROPERTY_ASSOCIATION", ["''", str("machining"), process, feature]);
    this.rel("MACHINING_FEATURE_RELATIONSHIP", ws, featureProcess);
    // Logical tool reference, not an invented ISO turning-tool body/holder model.
    var toolType = this.add("ACTION_RESOURCE_TYPE", [str("cutting tool")]);
    this.add("MACHINING_TOOL", [str(section.tool.number), str(section.tool.description), list([operation]), toolType]);
    this.textProperty(operation, "next-nc tool offset", String(section.tool.offset));
    this.textProperty(operation, "next-nc work offset", String(section.workOffset));
    this.property(operation, "next-nc entry point", this.rep(null, "", [this.point(section.start)], this.context));
    this.rel("MACHINING_TECHNOLOGY_RELATIONSHIP", operation, this.technology(section.initialSpindle, null));
    this.rel("MACHINING_FUNCTIONS_RELATIONSHIP", operation, this.functions(section.initialCoolant));
    for (var i = 0; i < section.paths.length; ++i) { this.toolpath(operation, section.paths[i], i + 1); }
  };
  Program.prototype.toSTEP = function () {
    requireValue(this.sections.length > 0, "program contains no sections");
    var w = new Writer(this.machine); w.units(this.units); var workplan = w.project(this);
    for (var i = 0; i < this.sections.length; ++i) { w.section(workplan, this.sections[i], i + 1); }
    this.lastExport = {entities: w.lines.length, reusedValues: w.reused, curveDefinitions: w.curves, sections: this.sections.length,
      paths: 0, arcs: 0, rapidSegments: 0, cuttingSegments: 0, dwells: 0};
    for (i = 0; i < this.sections.length; ++i) {
      for (var j = 0; j < this.sections[i].paths.length; ++j) {
        var path = this.sections[i].paths[j]; ++this.lastExport.paths;
        if (path.kind === "arc") { ++this.lastExport.arcs; }
        else if (path.kind === "dwell") { ++this.lastExport.dwells; }
        else { this.lastExport[path.kind === "rapid" ? "rapidSegments" : "cuttingSegments"] += path.points.length - 1; }
      }
    }
    var lines = ["ISO-10303-21;", "HEADER;",
      "FILE_DESCRIPTION((" + str("Experimental AP238 toolpath; " + this.profile) + "),'2;1');",
      "FILE_NAME(" + str(this.name + ".stpnc") + "," + str(this.timestamp) + ",(''),('')," + str("Fusion360Next-NC " + VERSION) + ",'Fusion360Next-NC','');",
      "FILE_SCHEMA(('INTEGRATED_CNC_SCHEMA'));", "ENDSEC;", "DATA;"];
    for (i = 0; i < w.lines.length; ++i) { lines.push("#" + (i + 1) + "=" + w.lines[i] + ";"); }
    lines.push("ENDSEC;", "END-ISO-10303-21;"); return lines.join("\n") + "\n";
  };
  return {Program: Program, version: VERSION, profile: PROFILE, millingProfile: MILL_PROFILE, stepString: str, stepReal: real};
}());
if (typeof module !== "undefined" && module.exports) { module.exports = NextNC; }
