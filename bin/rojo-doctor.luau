--!strict
local process = require("@lune/process")
local serde = require("@lune/serde")
local stdio = require("@lune/stdio")
local RojoDoctor = require("../luau/rojo_doctor")
local Types = require("../luau/types")

type Report = Types.Report

local function printHelp()
	stdio.write([[
rojo-doctor - Diagnose structural problems in Rojo projects

Usage:
  rojo-doctor check [PROJECT] [OPTIONS]

Options:
  --all             Check every .project.json / .project.jsonc in the directory
  --format <fmt>    Output format: text (default) or json
  --fix             Automatically fix safe issues (e.g. remove orphaned .meta.json files)
  -h, --help        Print this help message
]])
end

local function plural(count: number): string
	return count == 1 and "" or "s"
end

local function printTextReport(report: Report)
	stdio.write(string.format("Checking %s...\n\n", report.project))
	local pathLabel = report.resolved_paths == 1 and "path" or "paths"
	stdio.write(string.format("✓ %d mapped %s resolved\n", report.resolved_paths, pathLabel))

	for _, diag in ipairs(report.diagnostics) do
		stdio.write("\n")
		stdio.write(string.format("%s[%s]\n", diag.severity, diag.code))
		stdio.write(string.format("  %s\n", diag.subject))
		stdio.write(string.format("  %s\n", diag.message))
		if diag.help then
			stdio.write("\n")
			stdio.write(string.format("  help: %s\n", diag.help))
		end
	end

	stdio.write("\n")
	stdio.write(
		string.format(
			"%d warning%s, %d error%s\n",
			report.warnings,
			plural(report.warnings),
			report.errors,
			plural(report.errors)
		)
	)
end

local args = process.args
local command = "check"
local projectPath: string? = nil
local checkAll = false
local format = "text"
local fix = false

local i = 1
while i <= #args do
	local arg = args[i]
	if arg == "-h" or arg == "--help" then
		printHelp()
		process.exit(0)
	elseif arg == "check" then
		command = "check"
	elseif arg == "--all" then
		checkAll = true
	elseif arg == "--fix" then
		fix = true
	elseif arg == "--format" then
		i += 1
		format = args[i] or "text"
	elseif arg:sub(1, 9) == "--format=" then
		format = arg:sub(10)
	elseif not (arg:sub(1, 1) == "-") and projectPath == nil then
		projectPath = arg
	end
	i += 1
end

if command ~= "check" then
	stdio.ewrite("unknown command: " .. command .. "\n")
	process.exit(2)
end

local reports: { Report } = {}

if checkAll then
	local ok, res = pcall(function()
		return RojoDoctor.checkAll(projectPath or ".", { fix = fix })
	end)
	if not ok then
		stdio.ewrite("error[project-load]\n  " .. tostring(res) .. "\n")
		process.exit(2)
	end
	reports = res
else
	local ok, res = pcall(function()
		return RojoDoctor.check(projectPath, { fix = fix })
	end)
	if not ok then
		stdio.ewrite("error[project-load]\n  " .. tostring(res) .. "\n")
		process.exit(2)
	end
	table.insert(reports, res)
end

if format == "json" then
	local payload = checkAll and reports or reports[1]
	local json = serde.encode("json", payload, true)
	-- In Luau, empty arrays {} are serialized as {} unless converted
	json = json:gsub('"diagnostics":%s*{}', '"diagnostics": []')
	stdio.write(json .. "\n")
else
	for index, report in ipairs(reports) do
		if index > 1 then
			stdio.write("\n")
		end
		printTextReport(report)
	end
end

local hasFailures = false
for _, r in ipairs(reports) do
	if r.warnings > 0 or r.errors > 0 then
		hasFailures = true
		break
	end
end

if hasFailures then
	process.exit(1)
else
	process.exit(0)
end
