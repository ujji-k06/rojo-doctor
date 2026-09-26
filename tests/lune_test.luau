--!strict
local fs = require("@lune/fs")
local RojoDoctor = require("../luau/rojo_doctor")

local function assertEq(actual: any, expected: any, msg: string?)
	if actual ~= expected then
		error(
			string.format("%s: expected %s, got %s", msg or "assertion failed", tostring(expected), tostring(actual)),
			2
		)
	end
end

local function runTests()
	print("Running Lune tests for rojo-doctor...")

	-- 1. valid-json
	local r1 = RojoDoctor.check("tests/fixtures/valid-json")
	assertEq(r1.resolved_paths, 2, "valid-json resolved_paths")
	assertEq(r1.warnings, 0, "valid-json warnings")
	assertEq(r1.errors, 0, "valid-json errors")

	-- 2. valid-jsonc
	local r2 = RojoDoctor.check("tests/fixtures/valid-jsonc")
	assertEq(r2.resolved_paths, 1, "valid-jsonc resolved_paths")
	assertEq(r2.warnings, 0, "valid-jsonc warnings")
	assertEq(r2.errors, 0, "valid-jsonc errors")

	-- 3. missing-path
	local r3 = RojoDoctor.check("tests/fixtures/missing-path")
	assertEq(r3.warnings, 1, "missing-path warnings")
	assertEq(r3.diagnostics[1].code, "missing-path", "missing-path code")

	-- 4. pesde-packages
	local r4 = RojoDoctor.check("tests/fixtures/pesde-packages")
	assertEq(r4.warnings, 1, "pesde-packages warnings")
	assertEq(r4.diagnostics[1].code, "missing-path", "pesde-packages code")
	local help = r4.diagnostics[1].help or ""
	assert(help:find("pesde install") ~= nil, "pesde-packages should suggest pesde install")

	-- 5. file-with-children
	local r5 = RojoDoctor.check("tests/fixtures/file-with-children")
	assertEq(r5.errors, 1, "file-with-children errors")
	assertEq(r5.diagnostics[1].code, "file-with-children", "file-with-children code")

	-- 6. missing-class
	local r6 = RojoDoctor.check("tests/fixtures/missing-class")
	assertEq(r6.errors, 1, "missing-class errors")
	assertEq(r6.diagnostics[1].code, "missing-class", "missing-class code")

	-- 7. class-on-non-folder
	local r7 = RojoDoctor.check("tests/fixtures/class-on-file")
	assertEq(r7.errors, 1, "class-on-non-folder errors")
	assertEq(r7.diagnostics[1].code, "class-on-non-folder", "class-on-non-folder code")

	-- 8. child-collision
	local r8 = RojoDoctor.check("tests/fixtures/child-collision")
	assertEq(r8.errors, 1, "child-collision errors")
	assertEq(r8.diagnostics[1].code, "child-collision", "child-collision code")

	-- 9. ambiguous-init
	local r9 = RojoDoctor.check("tests/fixtures/ambiguous-init")
	assertEq(r9.warnings, 1, "ambiguous-init warnings")
	assertEq(r9.diagnostics[1].code, "ambiguous-init", "ambiguous-init code")

	-- 10. orphan-meta
	local r10 = RojoDoctor.check("tests/fixtures/orphan-meta")
	assertEq(r10.warnings, 1, "orphan-meta warnings")
	assertEq(r10.diagnostics[1].code, "orphan-meta", "orphan-meta code")

	-- 11. script-context
	local r11 = RojoDoctor.check("tests/fixtures/script-context")
	assertEq(r11.warnings, 1, "script-context warnings")
	assertEq(r11.diagnostics[1].code, "script-context", "script-context code")

	-- 12. checkAll on multi-project
	local multi = RojoDoctor.checkAll("tests/fixtures/multi-project")
	assertEq(#multi, 2, "multi-project count")

	-- 13. fix removes orphan-meta
	local tempDir = "tests/fixtures/test-fix-lune"
	if fs.isDir(tempDir) then
		fs.removeDir(tempDir)
	end
	fs.writeDir(tempDir)
	fs.writeDir(tempDir .. "/src")
	fs.writeFile(
		tempDir .. "/default.project.json",
		'{"name": "Temp", "tree": {"$className": "DataModel", "ReplicatedStorage": {"$path": "src"}}}'
	)
	fs.writeFile(tempDir .. "/src/Ghost.meta.json", "{}")
	fs.writeFile(tempDir .. "/src/Real.luau", "return {}")
	fs.writeFile(tempDir .. "/src/Real.meta.json", "{}")

	local beforeReport = RojoDoctor.check(tempDir)
	assertEq(beforeReport.warnings, 1, "temp before fix has 1 warning")

	local fixedCount = RojoDoctor.fix(tempDir)
	assertEq(fixedCount, 1, "fixed 1 orphan meta")
	assertEq(fs.isFile(tempDir .. "/src/Ghost.meta.json"), false, "Ghost.meta.json removed")
	assertEq(fs.isFile(tempDir .. "/src/Real.meta.json"), true, "Real.meta.json kept")
	assertEq(fs.isFile(tempDir .. "/src/Real.luau"), true, "Real.luau kept")

	local afterReport = RojoDoctor.check(tempDir)
	assertEq(afterReport.warnings, 0, "temp after fix has 0 warnings")
	fs.removeDir(tempDir)

	print("✓ All 13 headless Lune test suites passed successfully!")
end

runTests()
