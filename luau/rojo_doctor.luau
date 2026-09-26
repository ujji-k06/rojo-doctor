--!strict
local Types = require("./types")
local Project = require("./project")
local Checks = require("./checks")

export type Diagnostic = Types.Diagnostic
export type Report = Types.Report
export type LoadedProject = Types.LoadedProject

local RojoDoctor = {}

export type CheckOptions = {
	fix: boolean?,
}

function RojoDoctor.loadProject(path: string?): LoadedProject
	return Project.loadProject(path)
end

function RojoDoctor.check(path: string?, options: CheckOptions?): Report
	local loaded = Project.loadProject(path)
	if options and options.fix then
		Checks.fixAll(loaded)
	end
	local diagnostics, resolvedPaths = Checks.runAll(loaded)

	local warnings = 0
	local errors = 0
	for _, d in ipairs(diagnostics) do
		if d.severity == "warning" then
			warnings += 1
		elseif d.severity == "error" then
			errors += 1
		end
	end

	local projectName = loaded.path:match("[^/]+$") or loaded.path

	return {
		project = projectName,
		resolved_paths = resolvedPaths,
		warnings = warnings,
		errors = errors,
		diagnostics = diagnostics,
	}
end

function RojoDoctor.checkAll(directory: string?, options: CheckOptions?): { Report }
	local dir = directory or "."
	local files = Project.discoverProjectFiles(dir)
	local reports = {}
	for _, file in ipairs(files) do
		table.insert(reports, RojoDoctor.check(file, options))
	end
	return reports
end

function RojoDoctor.fix(path: string?): number
	local loaded = Project.loadProject(path)
	return Checks.fixAll(loaded)
end

return RojoDoctor
