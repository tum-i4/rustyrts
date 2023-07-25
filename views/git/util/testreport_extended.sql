CREATE VIEW testreport_extended
AS
SELECT c.id                       as commit,
       c.commit_str,
       c.repo_id,

       retest_all_report.id       as retest_all_id,
       retest_all_report.duration as retest_all_duration,

       dynamic_report.id          as dynamic_id,
       dynamic_report.duration    as dynamic_duration,

       static_report.id           as static_id,
       static_report.duration     as static_duration

FROM "Commit" c,
     "TestReport" retest_all_report,
     "TestReport" dynamic_report,
     "TestReport" static_report

WHERE c.id = retest_all_report.commit_id
  AND c.id = dynamic_report.commit_id
  AND c.id = static_report.commit_id

  AND retest_all_report.name = 'cargo test'
  AND dynamic_report.name = 'cargo rustyrts dynamic'
  AND static_report.name = 'cargo rustyrts static'

  AND retest_all_report.has_errored = false
  AND dynamic_report.has_errored = false
  AND static_report.has_errored = false

-- filter reports that are not comparable
--AND not exists(SELECT * FROM "TestSuite" s WHERE s.report_id = retest_all_report.id AND s.crashed = True)
--AND not exists(SELECT * FROM "TestSuite" s WHERE s.report_id = dynamic_report.id AND s.crashed = True)
--AND not exists(SELECT * FROM "TestSuite" s WHERE s.report_id = static_report.id AND s.crashed = True)
;

CREATE VIEW testreport_parent_extended
AS
SELECT c.id                       as commit,
       c.commit_str,
       c.repo_id,

       retest_all_report.id       as retest_all_id,
       retest_all_report.duration as retest_all_duration,

       dynamic_report.id          as dynamic_id,
       dynamic_report.duration    as dynamic_duration,

       static_report.id           as static_id,
       static_report.duration     as static_duration

FROM "Commit" c,
     "TestReport" retest_all_report,
     "TestReport" dynamic_report,
     "TestReport" static_report

WHERE c.id = retest_all_report.commit_id
  AND c.id = dynamic_report.commit_id
  AND c.id = static_report.commit_id

  AND retest_all_report.name = 'cargo test - parent'
  AND dynamic_report.name = 'cargo rustyrts dynamic - parent'
  AND static_report.name = 'cargo rustyrts static - parent'

  AND retest_all_report.has_errored = false
  AND dynamic_report.has_errored = false
  AND static_report.has_errored = false

-- filter reports that are not comparable
--AND not exists(SELECT * FROM "TestSuite" s WHERE s.report_id = retest_all_report.id AND s.crashed = True)
--AND not exists(SELECT * FROM "TestSuite" s WHERE s.report_id = dynamic_report.id AND s.crashed = True)
--AND not exists(SELECT * FROM "TestSuite" s WHERE s.report_id = static_report.id AND s.crashed = True)
;