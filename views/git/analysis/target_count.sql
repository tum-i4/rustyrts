-- this view calculates how often a test by target (unit or integration) has been selected or failed
create materialized view target_count
AS
SELECT overview.commit,
       overview.target,

       -- we use distinct here in case there are multiple tests with same suite name and testcase name

       count(distinct overview.retest_all_testcase_id)    AS retest_all_count,

       count(distinct (SELECT testcase.id
              FROM "TestCase" testcase
              WHERE testcase.id = overview.retest_all_testcase_id
                AND testcase.status = 'FAILED')) AS retest_all_count_failed,

       count(distinct overview.dynamic_testcase_id)       AS dynamic_count,

       count(distinct (SELECT testcase.id
              FROM "TestCase" testcase
              WHERE testcase.id = overview.dynamic_testcase_id
                AND testcase.status = 'FAILED')) AS dynamic_count_failed,

       count(distinct overview.static_testcase_id)        AS static_count,

       count(distinct (SELECT testcase.id
              FROM "TestCase" testcase
              WHERE testcase.id = overview.static_testcase_id
                AND testcase.status = 'FAILED')) AS static_count_failed


FROM testcase_overview overview
GROUP BY overview.commit, overview.target, overview.retest_all_id, overview.dynamic_id,
         overview.static_id;