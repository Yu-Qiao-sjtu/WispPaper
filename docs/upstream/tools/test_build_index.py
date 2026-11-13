import unittest

from build_index import hunks, names, source_lines, verify_lines, link


class DiffIndexTests(unittest.TestCase):
    def test_add_delete_and_blank_line(self):
        diff = '--- a/a\n+++ b/a\n@@ -2,2 +2,2 @@\n-old\n-\n+new\n+\n'
        h = hunks(diff)[0]
        self.assertEqual(h['deleted'], [{'line': 2, 'text': 'old'}, {'line': 3, 'text': ''}])
        verify_lines(h['deleted'], b'first\nold\n\nlast\n')
        verify_lines(h['added'], b'first\nnew\n\nlast\n')

    def test_zero_count_and_no_final_newline(self):
        h = hunks('@@ -0,0 +1 @@\n+new\n\\ No newline at end of file\n')[0]
        self.assertEqual(h['old_range'], [0, 0])
        verify_lines(h['added'], b'new')
        self.assertEqual(link('sha', 'a', 0, 0), '—')

    def test_utf8_crlf_and_unicode_line_separator(self):
        data = '一\r\nA\u2028B\n'.encode()
        self.assertEqual(source_lines(data), ['一\r', 'A\u2028B'])
        verify_lines([{'line': 2, 'text': 'A\u2028B'}], data)

    def test_rename_with_spaces(self):
        self.assertEqual(names(b'R100\0old name\0new name\0M\0a\0'), [('R100', 'old name', 'new name'), ('M', 'a', 'a')])
        self.assertIn('/new%20name#L2-L3', link('sha', 'new name', 2, 2))

    def test_reject_mismatched_line_or_incomplete_hunk(self):
        with self.assertRaises(ValueError):
            verify_lines([{'line': 2, 'text': 'wrong'}], b'a\nb\n')
        with self.assertRaises(ValueError):
            hunks('@@ -1,2 +1 @@\n-a\n+b\n')

    def test_content_that_looks_like_diff_header(self):
        h = hunks('@@ -1 +1 @@\n---- old\n++++ new\n')[0]
        self.assertEqual(h['deleted'][0]['text'], '--- old')
        self.assertEqual(h['added'][0]['text'], '+++ new')


if __name__ == '__main__':
    unittest.main()
