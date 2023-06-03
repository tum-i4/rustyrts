CREATE VIEW mutant_testcase_extended
AS
SELECT testcase.*,
       testsuite.crashed,
       testsuite.name as testsuite_name,
       testsuite.mutant_id
FROM "MutantsTestCase" testcase,
     "MutantsTestSuite" testsuite
WHERE testcase.suite_id = testsuite.id;