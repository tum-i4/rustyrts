-- this view calculates some statistics about retest-all
create materialized view statistics
AS

(
with counts as (SELECT commit.id,
                       commit.repo_id,
                       count(distinct suite.id)                as suites,
                       sum(suite.total_count)                  as cases,
                       sum((SELECT count(distinct cas.id)
                            FROM "TestCase" cas
                            WHERE cas.suite_id = suite.id
                              and cas.target = 'UNIT'))        as unit,
                       sum((SELECT count(distinct cas.id)
                            FROM "TestCase" cas
                            WHERE cas.suite_id = suite.id
                              and cas.target = 'INTEGRATION')) as integration,
                       sum(suite.duration)                     as duration
                FROM "Commit" commit,
                     "TestReport" report,
                     "TestSuite" suite
                WHERE commit.id = report.commit_id
                  AND report.has_errored = false
                  AND report.name = 'cargo test'
                  and suite.report_id = report.id
                GROUP BY commit.id, commit.repo_id)

SELECT repo_id,
       avg(suites)      as suites,
       avg(cases)       as cases,
       avg(unit)        as unit,
       avg(integration) as integration,
       avg(duration)    as duration
FROM counts
GROUP BY repo_id);