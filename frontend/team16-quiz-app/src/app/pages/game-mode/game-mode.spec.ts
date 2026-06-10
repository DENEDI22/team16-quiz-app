import { ComponentFixture, TestBed } from '@angular/core/testing';

import { GameMode } from './game-mode';

describe('GameMode', () => {
  let component: GameMode;
  let fixture: ComponentFixture<GameMode>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [GameMode],
    }).compileComponents();

    fixture = TestBed.createComponent(GameMode);
    component = fixture.componentInstance;
    await fixture.whenStable();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });
});
