create materialized view testcases_count
AS
SELECT overview.commit,
       count(overview.retest_all_testcase_id)    AS retest_all_count,

       count((SELECT testcase.id
              FROM "TestCase" testcase
              WHERE testcase.id = overview.retest_all_testcase_id
                AND testcase.status = 'FAILED')) AS retest_all_count_failed,

       count(overview.dynamic_testcase_id)       AS dynamic_count,

       count((SELECT testcase.id
              FROM "TestCase" testcase
              WHERE testcase.id = overview.dynamic_testcase_id
                AND testcase.status = 'FAILED')) AS dynamic_count_failed,

       count(overview.static_testcase_id)        AS static_count,

       count((SELECT testcase.id
              FROM "TestCase" testcase
              WHERE testcase.id = overview.static_testcase_id
                AND testcase.status = 'FAILED')) AS static_count_failed


FROM testcase_overview overview
GROUP BY overview.commit, overview.retest_all_id, overview.dynamic_id,
         overview.static_id;