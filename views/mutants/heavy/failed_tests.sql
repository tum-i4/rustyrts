create materialized view failed_tests
AS
SELECT c.id                                     as commit,
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
          AND testcase.status = 'FAILED')       AS retest_all,

       (SELECT count(testcase.name)
                   AS tests
        FROM "MutantsTestSuite" testsuite,
             "MutantsTestCase" testcase
        WHERE testsuite.mutant_id = retest_all_mutant.id
          AND testsuite.id = testcase.suite_id) AS retest_all_count,

       (SELECT count(testcase.name)
                   AS tests
        FROM "MutantsTestSuite" testsuite,
             "MutantsTestCase" testcase
        WHERE testsuite.mutant_id = retest_all_mutant.id
          AND testsuite.id = testcase.suite_id
          AND testcase.status = 'FAILED')       AS retest_all_count_failed,


       (SELECT coalesce(STRING_AGG(testcase.name,
                                   E'\n'
                                   ORDER BY testcase.name), '')
                   AS tests
        FROM "MutantsTestSuite" testsuite,
             "MutantsTestCase" testcase
        WHERE testsuite.mutant_id = dynamic_mutant.id
          AND testsuite.id = testcase.suite_id
          AND testcase.status = 'FAILED')       AS dynamic,

       (SELECT count(testcase.name)
                   AS tests
        FROM "MutantsTestSuite" testsuite,
             "MutantsTestCase" testcase
        WHERE testsuite.mutant_id = dynamic_mutant.id
          AND testsuite.id = testcase.suite_id) AS dynamic_count,

       (SELECT count(testcase.name)
                   AS tests
        FROM "MutantsTestSuite" testsuite,
             "MutantsTestCase" testcase
        WHERE testsuite.mutant_id = dynamic_mutant.id
          AND testsuite.id = testcase.suite_id
          AND testcase.status = 'FAILED')       AS dynamic_count_failed,

       (SELECT coalesce(STRING_AGG(testcase.name,
                                   E'\n'
                                   ORDER BY testcase.name), '')
                   AS tests
        FROM "MutantsTestSuite" testsuite,
             "MutantsTestCase" testcase
        WHERE testsuite.mutant_id = static_mutant.id
          AND testsuite.id = testcase.suite_id
          AND testcase.status = 'FAILED')       AS static,

       (SELECT count(testcase.name)
                   AS tests
        FROM "MutantsTestSuite" testsuite,
             "MutantsTestCase" testcase
        WHERE testsuite.mutant_id = static_mutant.id
          AND testsuite.id = testcase.suite_id) AS static_count,

       (SELECT count(testcase.name)
                   AS tests
        FROM "MutantsTestSuite" testsuite,
             "MutantsTestCase" testcase
        WHERE testsuite.mutant_id = static_mutant.id
          AND testsuite.id = testcase.suite_id
          AND testcase.status = 'FAILED')       AS static_count_failed

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