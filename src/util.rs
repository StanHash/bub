/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

pub fn sorted_merge<T>(a: &[T], b: &[T]) -> Vec<T>
    where T: Ord + Copy
{
    if a.len() == 0 { return b.into() }
    if b.len() == 0 { return a.into() }

    let (mut ia, mut ib) = (0, 0);
    let mut sorted = Vec::with_capacity(a.len() + b.len());

    let remaining;
    let remaining_begin;

    loop
    {
        if a[ia] < b[ib]
        {
            sorted.push(a[ia]);
            ia += 1;

            if ia == a.len()
            {
                remaining = b;
                remaining_begin = ib;
                break;
            }
        }
        else
        {
            sorted.push(b[ib]);
            ib += 1;

            if ib == b.len()
            {
                remaining = a;
                remaining_begin = ia;
                break;
            }
        }
    }

    for i in remaining_begin .. remaining.len() {
        sorted.push(remaining[i]); }

    sorted
}
