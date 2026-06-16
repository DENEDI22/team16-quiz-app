import { ComponentFixture, TestBed } from '@angular/core/testing';

import { Agb } from './agb';

describe('Agb', () => {
  let component: Agb;
  let fixture: ComponentFixture<Agb>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [Agb],
    }).compileComponents();

    fixture = TestBed.createComponent(Agb);
    component = fixture.componentInstance;
    await fixture.whenStable();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });
});
