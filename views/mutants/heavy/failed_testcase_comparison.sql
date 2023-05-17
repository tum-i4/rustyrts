create materialized view failed_comparison
AS
SELECT c.id    as commit,
           c.commit_str,
           c.repo_id,
           retest_all_mutant.descr,

           (SELECT coalesce(STRING_AGG(testcase.name,
                                       E'\n'
                                       ORDER BY testcase.name), '')
                       AS tests
            FROM "MutantsTestSuite" testsuite,
                 "MutantsTestCase" testcase
            WHERE testsuite.mutant_id = retest_all_mutant.id
              AND testsuite.id = testcase.suite_id
              AND testcase.status = 'FAILED'
              AND testsuite.crashed = false -- Limit to test suites that did not crash
              AND not exists(
                    SELECT *
                    FROM "MutantsTestSuite" dynamic_testsuite,
                         "MutantsTestCase" dynamic_testcase
                    WHERE dynamic_testsuite.mutant_id = dynamic_mutant.id
                      AND dynamic_testsuite.id = dynamic_testcase.suite_id
--                AND dynamic_testcase.status = 'FAILED' -- we do not care for the outcome on the dynamic_mutant, only that the test has been selected
                      AND dynamic_testsuite.name = testsuite.name
                      AND dynamic_testcase.name = testcase.name
                )) AS failed_but_not_selected_dynamic,


           (SELECT coalesce(STRING_AGG(testcase.name,
                                       E'\n'
                                       ORDER BY testcase.name), '')
                       AS tests
            FROM "MutantsTestSuite" testsuite,
                 "MutantsTestCase" testcase
            WHERE testsuite.mutant_id = retest_all_mutant.id
              AND testsuite.id = testcase.suite_id
              AND testcase.status = 'FAILED'
              AND testsuite.crashed = false -- Limit to test suites that did not crash
              AND not exists(
                    SELECT *
                    FROM "MutantsTestSuite" static_testsuite,
                         "MutantsTestCase" static_testcase
                    WHERE static_testsuite.mutant_id = static_mutant.id
                      AND static_testsuite.id = static_testcase.suite_id
--                AND static_testcase.status = 'FAILED' -- we do not care for the outcome on the static_mutant, only that the test has been selected
                      AND static_testsuite.name = testsuite.name
                      AND static_testcase.name = testcase.name
                )) AS failed_but_not_selected_static

    FROM "Commit" c,
         "MutantsReport" retest_all,
         "MutantsReport" dynamic,
         "MutantsReport" static,
         "Mutant" retest_all_mutant,
         "Mutant" dynamic_mutant,
         "Mutant" static_mutant
    WHERE c.id = retest_all.commit_id
      AND c.id = dynamic.commit_id
      AND c.id = static.commit_id

      AND retest_all_mutant.report_id = retest_all.id
      AND dynamic_mutant.report_id = dynamic.id
      AND static_mutant.report_id = static.id

      AND retest_all.name = 'mutants'
      AND dynamic.name = 'mutants dynamic'
      AND static.name = 'mutants static'

      AND retest_all_mutant.descr = dynamic_mutant.descr
      AND retest_all_mutant.descr = static_mutant.descr

      AND retest_all_mutant.test_log is not null

      AND retest_all_mutant.test_result != 'TIMEOUT'
      AND dynamic_mutant.test_result != 'TIMEOUT'
      AND static_mutant.test_result != 'TIMEOUT';